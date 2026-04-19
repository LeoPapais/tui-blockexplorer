//! Exponential-backoff retry helper with jitter.
//!
//! Shared by [`super::RpcClient`] and (eventually) the WebSocket
//! reconnect loop in [`super::pending_tx_stream`] /
//! [`super::new_heads_stream`]. See
//! `plan/13-alchemy-adapter.md` §8.2 and
//! `.cursor/rules/external-apis.mdc` ("Rate limits and retries").

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use crate::application::ports::Rng;

use super::client::RpcError;

/// How many total attempts (initial + retries) the policy makes.
/// The rules mandate 3 attempts total (initial + 2 retries).
pub const DEFAULT_MAX_ATTEMPTS: usize = 3;
/// Base unit of delay. At attempt `n` the raw delay is
/// `DEFAULT_BASE_DELAY * 2^n`; the actual delay adds a random
/// jitter draw from `[0, base_delay)`.
pub const DEFAULT_BASE_DELAY: Duration = Duration::from_millis(200);
/// Ceiling on the computed delay so a pathological power-of-two blow
/// up cannot suspend a request forever.
pub const DEFAULT_MAX_DELAY: Duration = Duration::from_secs(5);

/// Jittered exponential-backoff policy driven by an injected [`Rng`]
/// port so the stream of delays is deterministic under test.
#[derive(Clone)]
pub struct RetryPolicy {
    max_attempts: usize,
    base_delay: Duration,
    max_delay: Duration,
    rng: Arc<dyn Rng>,
}

impl RetryPolicy {
    /// Build a policy with the given attempts + delays and the
    /// supplied [`Rng`] port. `max_attempts == 1` disables retries.
    ///
    /// # Panics
    ///
    /// Panics if `max_attempts == 0`. A policy with zero attempts
    /// would never call the operation, which is almost certainly a
    /// bug — use `RetryPolicy::none()` to disable retries instead.
    #[must_use]
    pub fn new(
        max_attempts: usize,
        base_delay: Duration,
        max_delay: Duration,
        rng: Arc<dyn Rng>,
    ) -> Self {
        assert!(
            max_attempts >= 1,
            "RetryPolicy::new: max_attempts must be >= 1; use RetryPolicy::none() to disable retries",
        );
        Self {
            max_attempts,
            base_delay,
            max_delay,
            rng,
        }
    }

    /// Default hardening policy: 3 attempts, 200 ms base delay, 5 s
    /// ceiling. Matches the numbers recorded in
    /// `plan/13-alchemy-adapter.md` §8.2.
    #[must_use]
    pub fn default_with_rng(rng: Arc<dyn Rng>) -> Self {
        Self::new(
            DEFAULT_MAX_ATTEMPTS,
            DEFAULT_BASE_DELAY,
            DEFAULT_MAX_DELAY,
            rng,
        )
    }

    /// Policy that issues exactly one attempt and never retries.
    /// Used when an adapter wants to opt out at construction time.
    #[must_use]
    pub fn none() -> Self {
        Self {
            max_attempts: 1,
            base_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
            rng: Arc::new(NoopRng),
        }
    }

    /// Expose the configured attempt count so callers (tests,
    /// circuit breaker) can reason about the ceiling.
    #[must_use]
    pub fn max_attempts(&self) -> usize {
        self.max_attempts
    }

    /// Delay before the attempt **after** `attempt_index`, where
    /// `attempt_index` is 0-based (0 means "we just finished the
    /// initial attempt and are about to start retry #1").
    ///
    /// Formula: `min(base * 2^attempt_index + jitter, max_delay)`,
    /// where `jitter` is drawn uniformly from `[0, base_delay)`.
    /// Returns `Duration::ZERO` when `max_attempts == 1`.
    #[must_use]
    pub fn backoff_for(&self, attempt_index: usize) -> Duration {
        if self.base_delay.is_zero() {
            return Duration::ZERO;
        }
        let shift = u32::try_from(attempt_index).unwrap_or(u32::MAX);
        let factor = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
        let base_nanos = u64::try_from(self.base_delay.as_nanos()).unwrap_or(u64::MAX);
        let raw = base_nanos.saturating_mul(factor);
        let jitter = self.jitter_nanos();
        let total = raw.saturating_add(jitter);
        let capped = total.min(u64::try_from(self.max_delay.as_nanos()).unwrap_or(u64::MAX));
        Duration::from_nanos(capped)
    }

    fn jitter_nanos(&self) -> u64 {
        let base_nanos = u64::try_from(self.base_delay.as_nanos()).unwrap_or(u64::MAX);
        if base_nanos == 0 {
            return 0;
        }
        let draw = self.rng.next_u64();
        draw % base_nanos
    }
}

impl std::fmt::Debug for RetryPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetryPolicy")
            .field("max_attempts", &self.max_attempts)
            .field("base_delay", &self.base_delay)
            .field("max_delay", &self.max_delay)
            .finish_non_exhaustive()
    }
}

/// Run `operation` under the given [`RetryPolicy`]. Retries are
/// driven by [`RpcError::is_retryable`]; non-retryable errors
/// surface immediately and short-circuit the loop.
///
/// The operation is re-invoked from scratch on every attempt, which
/// matches the semantics every JSON-RPC / HTTP caller in this crate
/// expects (a fresh `reqwest::RequestBuilder::send` per try).
pub async fn retry_with_backoff<F, Fut, T>(
    policy: &RetryPolicy,
    mut operation: F,
) -> Result<T, RpcError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, RpcError>>,
{
    debug_assert!(policy.max_attempts >= 1);
    let mut last_err: Option<RpcError> = None;
    for attempt_index in 0..policy.max_attempts {
        if attempt_index > 0 {
            let delay = policy.backoff_for(attempt_index - 1);
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
        }
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                if !err.is_retryable() {
                    return Err(err);
                }
                last_err = Some(err);
            }
        }
    }
    // unreachable: `max_attempts >= 1` guarantees the loop ran at
    // least once; if the first attempt succeeded we already returned.
    Err(last_err.unwrap_or(RpcError::Timeout))
}

/// `Rng` impl used by [`RetryPolicy::none()`]. Never consulted
/// because the associated `base_delay` is zero, but Rust still needs
/// a concrete type behind the `Arc<dyn Rng>`.
struct NoopRng;

impl Rng for NoopRng {
    fn fill_bytes(&self, dest: &mut [u8]) {
        for byte in dest.iter_mut() {
            *byte = 0;
        }
    }
    fn next_u64(&self) -> u64 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ConstantRng(u64);
    impl Rng for ConstantRng {
        fn fill_bytes(&self, dest: &mut [u8]) {
            for byte in dest.iter_mut() {
                *byte = 0;
            }
        }
        fn next_u64(&self) -> u64 {
            self.0
        }
    }

    fn policy_with_rng(rng: Arc<dyn Rng>) -> RetryPolicy {
        RetryPolicy::new(3, Duration::from_millis(100), Duration::from_secs(5), rng)
    }

    #[test]
    fn backoff_for_honors_max_delay_ceiling() {
        let rng = Arc::new(ConstantRng(0));
        let policy = RetryPolicy::new(10, Duration::from_millis(1), Duration::from_millis(4), rng);
        // 1ms * 2^30 is way above 4ms ceiling.
        assert_eq!(policy.backoff_for(30), Duration::from_millis(4));
    }

    #[test]
    fn backoff_for_is_zero_when_policy_has_zero_base_delay() {
        let policy = RetryPolicy::none();
        assert_eq!(policy.backoff_for(0), Duration::ZERO);
        assert_eq!(policy.backoff_for(5), Duration::ZERO);
    }

    #[test]
    fn backoff_for_stays_within_expected_window() {
        let rng = Arc::new(ConstantRng(u64::MAX));
        let policy = policy_with_rng(rng);
        // attempt 0: 100ms + up to 100ms jitter -> < 200ms
        let d = policy.backoff_for(0);
        assert!(d >= Duration::from_millis(100) && d < Duration::from_millis(200));
        // attempt 1: 200ms + up to 100ms jitter -> < 300ms
        let d = policy.backoff_for(1);
        assert!(d >= Duration::from_millis(200) && d < Duration::from_millis(300));
    }

    #[test]
    #[should_panic(expected = "max_attempts must be >= 1")]
    fn new_rejects_zero_attempts() {
        let _ = RetryPolicy::new(
            0,
            Duration::from_millis(1),
            Duration::from_millis(1),
            Arc::new(ConstantRng(0)),
        );
    }

    #[tokio::test]
    async fn retry_helper_caps_attempts_at_three() {
        let rng = Arc::new(ConstantRng(0));
        let policy = RetryPolicy::new(3, Duration::from_millis(1), Duration::from_millis(2), rng);

        let call_count = std::sync::atomic::AtomicUsize::new(0);
        let result: Result<(), _> = retry_with_backoff(&policy, || {
            call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async { Err::<(), _>(RpcError::Rate) }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retry_helper_returns_early_on_non_retryable_error() {
        let rng = Arc::new(ConstantRng(0));
        let policy = RetryPolicy::new(3, Duration::from_millis(1), Duration::from_millis(2), rng);

        let call_count = std::sync::atomic::AtomicUsize::new(0);
        let result: Result<(), _> = retry_with_backoff(&policy, || {
            call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async {
                Err::<(), _>(RpcError::Rpc {
                    code: -32602,
                    message: "invalid params".into(),
                })
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(
            call_count.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "non-retryable error must not burn retries",
        );
    }

    #[tokio::test]
    async fn retry_helper_recovers_after_transient_failures() {
        let rng = Arc::new(ConstantRng(0));
        let policy = RetryPolicy::new(3, Duration::from_millis(1), Duration::from_millis(2), rng);

        let call_count = std::sync::atomic::AtomicUsize::new(0);
        let result: Result<&'static str, _> = retry_with_backoff(&policy, || {
            let n = call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async move {
                match n {
                    0 => Err(RpcError::Rate),
                    1 => Err(RpcError::HttpServerError { status: 503 }),
                    _ => Ok("ok"),
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), "ok");
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 3);
    }
}
