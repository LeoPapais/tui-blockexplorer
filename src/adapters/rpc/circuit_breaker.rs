//! Per-adapter circuit breaker. See `plan/13-alchemy-adapter.md` §8.3
//! and `plan/15-backlog.md` §8.14 item 3.
//!
//! The breaker lives on the [`super::RpcClient`] side so every
//! adapter sharing the same client shares the same breaker. The
//! breaker is pure logic + a [`Clock`] port; no async, no locks
//! contended more than a few microseconds per transition.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::application::ports::Clock;

use super::client::RpcError;

/// Failure threshold + cool-down configuration for [`CircuitBreaker`].
#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerConfig {
    /// How many consecutive retryable failures trip the breaker.
    pub failure_threshold: usize,
    /// How long the breaker stays open before allowing a probe call.
    pub cool_down: Duration,
}

impl CircuitBreakerConfig {
    /// Default configuration: 5 consecutive failures, 30 s cool-down.
    /// Matches the "provider circuit breaker" rule of thumb in
    /// `.cursor/rules/external-apis.mdc`.
    pub const DEFAULT: Self = Self {
        failure_threshold: 5,
        cool_down: Duration::from_secs(30),
    };
}

/// Three-state machine: `Closed` (happy path), `Open` (fast-fail
/// with [`RpcError::CircuitOpen`]) and `HalfOpen` (one probe call
/// allowed). The state transitions are:
///
/// * `Closed` + `record_failure` past the threshold → `Open`.
/// * `Open` after `cool_down` elapses on `is_open` check → `HalfOpen`.
/// * `HalfOpen` + `record_success` → `Closed`.
/// * `HalfOpen` + `record_failure` → `Open` (reset the clock).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BreakerState {
    Closed { consecutive_failures: usize },
    Open { opened_at: Instant },
    HalfOpen,
}

/// Per-adapter circuit breaker.
///
/// Shared through `Arc` so a single instance can gate many calls.
/// Failures are counted at the *logical* call level (one attempt
/// from the caller's POV, which may internally expand into several
/// HTTP retries through the retry helper).
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    clock: Arc<dyn Clock>,
    state: Mutex<BreakerState>,
}

impl CircuitBreaker {
    /// Build a breaker from a configuration + clock.
    #[must_use]
    pub fn new(config: CircuitBreakerConfig, clock: Arc<dyn Clock>) -> Self {
        Self {
            config,
            clock,
            state: Mutex::new(BreakerState::Closed {
                consecutive_failures: 0,
            }),
        }
    }

    /// Convenience: default config ([`CircuitBreakerConfig::DEFAULT`])
    /// + supplied clock.
    #[must_use]
    pub fn default_with_clock(clock: Arc<dyn Clock>) -> Self {
        Self::new(CircuitBreakerConfig::DEFAULT, clock)
    }

    /// Is the breaker currently rejecting calls?
    ///
    /// Performs the `Open -> HalfOpen` lazy transition: when the
    /// cool-down has elapsed the breaker flips to `HalfOpen` and
    /// returns `false` so exactly one probe call is let through.
    #[must_use]
    pub fn is_open(&self) -> bool {
        let mut guard = self.state.lock().expect("breaker state lock poisoned");
        match *guard {
            BreakerState::Closed { .. } | BreakerState::HalfOpen => false,
            BreakerState::Open { opened_at } => {
                let now = self.clock.now();
                if now.duration_since(opened_at) >= self.config.cool_down {
                    *guard = BreakerState::HalfOpen;
                    false
                } else {
                    true
                }
            }
        }
    }

    /// Record a successful call, resetting the failure counter and
    /// moving the breaker back to `Closed`.
    pub fn record_success(&self) {
        let mut guard = self.state.lock().expect("breaker state lock poisoned");
        *guard = BreakerState::Closed {
            consecutive_failures: 0,
        };
    }

    /// Record a retryable failure. When the threshold is reached the
    /// breaker opens with `opened_at` pinned to the current `Clock`
    /// sample.
    pub fn record_failure(&self) {
        let mut guard = self.state.lock().expect("breaker state lock poisoned");
        let next = match *guard {
            BreakerState::Closed {
                consecutive_failures,
            } => {
                let bumped = consecutive_failures + 1;
                if bumped >= self.config.failure_threshold {
                    BreakerState::Open {
                        opened_at: self.clock.now(),
                    }
                } else {
                    BreakerState::Closed {
                        consecutive_failures: bumped,
                    }
                }
            }
            BreakerState::HalfOpen => BreakerState::Open {
                opened_at: self.clock.now(),
            },
            BreakerState::Open { .. } => *guard,
        };
        *guard = next;
    }
}

impl std::fmt::Debug for CircuitBreaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CircuitBreaker")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// Check whether an error caused by the underlying transport should
/// be counted as a breaker failure. Non-retryable errors
/// (InvalidInput, FeatureUnavailable, ...) do **not** trip the
/// breaker — otherwise a method typo would knock out the whole
/// provider.
#[must_use]
pub fn is_breaker_failure(err: &RpcError) -> bool {
    err.is_retryable()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct ManualClock(Arc<Mutex<Instant>>);

    impl ManualClock {
        fn new() -> Self {
            Self(Arc::new(Mutex::new(Instant::now())))
        }

        fn advance(&self, delta: Duration) {
            *self.0.lock().unwrap() += delta;
        }
    }

    impl Clock for ManualClock {
        fn now(&self) -> Instant {
            *self.0.lock().unwrap()
        }
    }

    fn breaker_for_tests(clock: Arc<ManualClock>) -> CircuitBreaker {
        CircuitBreaker::new(
            CircuitBreakerConfig {
                failure_threshold: 3,
                cool_down: Duration::from_secs(10),
            },
            clock,
        )
    }

    #[test]
    fn starts_closed_and_lets_calls_through() {
        let clock = Arc::new(ManualClock::new());
        let breaker = breaker_for_tests(clock);
        assert!(!breaker.is_open());
    }

    #[test]
    fn opens_after_threshold_consecutive_failures() {
        let clock = Arc::new(ManualClock::new());
        let breaker = breaker_for_tests(clock.clone());
        for _ in 0..2 {
            breaker.record_failure();
        }
        assert!(!breaker.is_open(), "below threshold stays closed");
        breaker.record_failure();
        assert!(breaker.is_open(), "threshold met should open");
    }

    #[test]
    fn success_resets_counter_before_threshold() {
        let clock = Arc::new(ManualClock::new());
        let breaker = breaker_for_tests(clock);
        breaker.record_failure();
        breaker.record_failure();
        breaker.record_success();
        breaker.record_failure();
        breaker.record_failure();
        assert!(!breaker.is_open());
    }

    #[test]
    fn open_transitions_to_half_open_after_cool_down() {
        let clock = Arc::new(ManualClock::new());
        let breaker = breaker_for_tests(clock.clone());
        for _ in 0..3 {
            breaker.record_failure();
        }
        assert!(breaker.is_open());

        clock.advance(Duration::from_secs(10));
        assert!(
            !breaker.is_open(),
            "after cool-down the breaker must allow a probe call",
        );
    }

    #[test]
    fn half_open_success_returns_to_closed() {
        let clock = Arc::new(ManualClock::new());
        let breaker = breaker_for_tests(clock.clone());
        for _ in 0..3 {
            breaker.record_failure();
        }
        clock.advance(Duration::from_secs(10));
        assert!(!breaker.is_open()); // → HalfOpen
        breaker.record_success();
        // Back to Closed: threshold counter reset.
        for _ in 0..2 {
            breaker.record_failure();
        }
        assert!(!breaker.is_open());
    }

    #[test]
    fn half_open_failure_re_opens_with_fresh_cool_down() {
        let clock = Arc::new(ManualClock::new());
        let breaker = breaker_for_tests(clock.clone());
        for _ in 0..3 {
            breaker.record_failure();
        }
        clock.advance(Duration::from_secs(10));
        assert!(!breaker.is_open()); // → HalfOpen
        breaker.record_failure();
        assert!(breaker.is_open(), "failed probe re-opens the breaker");
    }
}
