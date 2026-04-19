//! Generic JSON-RPC 2.0 client for Alchemy HTTP endpoints.
//!
//! See `plan/13-alchemy-adapter.md` section 2.

use std::sync::Arc;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use thiserror::Error;
use url::Url;

use super::circuit_breaker::{CircuitBreaker, is_breaker_failure};
use super::cost_hint::cost_hint_for;
use super::retry::{RetryPolicy, retry_with_backoff};
use crate::application::ports::Rng;
use crate::domain::DomainError;

/// Trait implemented by the [`crate::infra::cost_meter::CostMeter`];
/// declared here so the adapter layer does not depend on `infra`.
/// See `plan/13-alchemy-adapter.md` §8.4.
pub trait CostRecorder: Send + Sync {
    fn record(&self, compute_units: u32);
}

/// Errors produced by the RPC client. Mapped to [`DomainError`] at the
/// adapter boundary via [`RpcError::into_domain`].
#[derive(Debug, Error)]
pub enum RpcError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// HTTP 5xx response from the provider. Split out of `Rate` so the
    /// retry classifier (plan/13 §8.2) can tell "our fault vs theirs".
    #[error("HTTP server error: {status}")]
    HttpServerError { status: u16 },

    #[error("JSON-RPC error {code}: {message}")]
    Rpc { code: i64, message: String },

    #[error("response could not be decoded: {0}")]
    Decode(#[from] serde_json::Error),

    /// HTTP 429 Too Many Requests. Distinct from `HttpServerError`
    /// so the breaker + retry helpers can weigh them differently.
    #[error("rate limited")]
    Rate,

    #[error("request timed out")]
    Timeout,

    /// The circuit breaker fast-failed the call. Mapped to
    /// `DomainError::ProviderUnavailable` at the adapter boundary.
    /// See `plan/13-alchemy-adapter.md` §8.3.
    #[error("circuit breaker open; fast-failing the call")]
    CircuitOpen,
}

impl RpcError {
    /// Map into the domain error model at the adapter boundary.
    ///
    /// See `plan/13-alchemy-adapter.md` §8.1 for the matrix:
    /// - `-32602` (invalid params) → `DomainError::InvalidInput`.
    /// - `-32601` / `-32004` (method not found / not supported) →
    ///   `DomainError::FeatureUnavailable`.
    /// - `-32005` (rate limit) and `-32000..=-32099` (server error
    ///   range) → `DomainError::ProviderUnavailable`.
    /// - Anything else from the RPC envelope falls through to
    ///   `Internal(message)` so regressions surface instead of hiding
    ///   as a generic provider error.
    #[must_use]
    pub fn into_domain(self) -> DomainError {
        match self {
            RpcError::Rate
            | RpcError::Timeout
            | RpcError::HttpServerError { .. }
            | RpcError::CircuitOpen => DomainError::ProviderUnavailable,
            RpcError::Http(err) if err.is_timeout() => DomainError::ProviderUnavailable,
            RpcError::Http(err) if err.is_connect() => DomainError::ProviderUnavailable,
            RpcError::Http(err) => DomainError::Internal(err.to_string()),
            RpcError::Rpc { code: -32602, message } => DomainError::InvalidInput(message),
            RpcError::Rpc {
                code: -32601 | -32004,
                ..
            } => DomainError::FeatureUnavailable,
            RpcError::Rpc {
                code: -32099..=-32000,
                ..
            } => DomainError::ProviderUnavailable,
            RpcError::Rpc { message, .. } => DomainError::Internal(message),
            RpcError::Decode(err) => DomainError::Internal(err.to_string()),
        }
    }

    /// True when the error is transient and retrying the same call
    /// may succeed. Drives both the retry helper (plan/13 §8.2) and
    /// the circuit breaker's failure counter (§8.3).
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            RpcError::Rate
            | RpcError::Timeout
            | RpcError::HttpServerError { .. }
            | RpcError::Rpc { code: -32005, .. } => true,
            RpcError::Http(err) => err.is_timeout() || err.is_connect(),
            _ => false,
        }
    }
}

/// Thin JSON-RPC 2.0 client. One instance is reused across calls on the
/// same base URL.
#[derive(Clone)]
pub struct RpcClient {
    http: Client,
    base_url: Url,
    retry: Arc<RetryPolicy>,
    breaker: Option<Arc<CircuitBreaker>>,
    cost_recorder: Option<Arc<dyn CostRecorder>>,
}

impl std::fmt::Debug for RpcClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcClient")
            .field("base_url", &self.base_url)
            .field("retry", &self.retry)
            .field("breaker", &self.breaker)
            .finish_non_exhaustive()
    }
}

impl RpcClient {
    /// Construct a new client. Callers build the `reqwest::Client` once
    /// and pass it in so connection pooling is shared across adapters.
    ///
    /// The returned client uses [`RetryPolicy::none()`] and has no
    /// circuit breaker; opt in via [`Self::with_default_retry`] /
    /// [`Self::with_retry_policy`] and [`Self::with_circuit_breaker`].
    #[must_use]
    pub fn new(base_url: Url, http: Client) -> Self {
        Self {
            http,
            base_url,
            retry: Arc::new(RetryPolicy::none()),
            breaker: None,
            cost_recorder: None,
        }
    }

    /// Convenience constructor: builds a `reqwest::Client` with sane
    /// defaults (10 second timeout, rustls-only TLS).
    pub fn with_default_http(base_url: Url) -> Result<Self, RpcError> {
        let http = Client::builder().timeout(Duration::from_secs(10)).build()?;
        Ok(Self::new(base_url, http))
    }

    /// Consume `self` and attach an explicit [`RetryPolicy`]. Prefer
    /// [`Self::with_default_retry`] unless a test needs a tailored
    /// delay schedule.
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry = Arc::new(policy);
        self
    }

    /// Convenience for the MVP composition root: install the default
    /// 3-attempt jittered policy using the supplied [`Rng`] port.
    #[must_use]
    pub fn with_default_retry(self, rng: Arc<dyn Rng>) -> Self {
        self.with_retry_policy(RetryPolicy::default_with_rng(rng))
    }

    /// Retry policy currently attached to this client. Exposed for
    /// the circuit breaker + tests; do not mutate through it.
    #[must_use]
    pub fn retry_policy(&self) -> &RetryPolicy {
        &self.retry
    }

    /// Attach a shared circuit breaker. See
    /// `plan/13-alchemy-adapter.md` §8.3: after
    /// `failure_threshold` consecutive retryable failures the
    /// breaker opens and every subsequent call short-circuits with
    /// [`RpcError::CircuitOpen`] until `cool_down` elapses.
    #[must_use]
    pub fn with_circuit_breaker(mut self, breaker: Arc<CircuitBreaker>) -> Self {
        self.breaker = Some(breaker);
        self
    }

    /// Currently-attached circuit breaker, if any. Exposed so the
    /// composite signature directory (plan/15-backlog.md §3.2) and
    /// tests can consult `is_open()` without a second Arc hop.
    #[must_use]
    pub fn circuit_breaker(&self) -> Option<&Arc<CircuitBreaker>> {
        self.breaker.as_ref()
    }

    /// Install a [`CostRecorder`] (typically the process-level
    /// `CostMeter` in `infra`). Every successful `call` charges the
    /// recorder using the hint returned by [`cost_hint_for`].
    /// See `plan/13-alchemy-adapter.md` §8.4.
    #[must_use]
    pub fn with_cost_recorder(mut self, recorder: Arc<dyn CostRecorder>) -> Self {
        self.cost_recorder = Some(recorder);
        self
    }

    /// Issue a JSON-RPC 2.0 call and decode the `result` field.
    pub async fn call<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R, RpcError> {
        if let Some(breaker) = &self.breaker
            && breaker.is_open()
        {
            return Err(RpcError::CircuitOpen);
        }

        let params_value = serde_json::to_value(&params)?;
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params_value,
        });

        let outcome = retry_with_backoff(&self.retry, || self.call_once::<R>(&request)).await;
        if let Some(breaker) = &self.breaker {
            match &outcome {
                Ok(_) => breaker.record_success(),
                Err(err) if is_breaker_failure(err) => breaker.record_failure(),
                Err(_) => {}
            }
        }
        if outcome.is_ok()
            && let Some(recorder) = &self.cost_recorder
        {
            recorder.record(cost_hint_for(method).compute_units);
        }
        outcome
    }

    async fn call_once<R: DeserializeOwned>(&self, request: &Value) -> Result<R, RpcError> {
        let body = self.post_and_parse(request).await?;
        parse_single_rpc_envelope(body)
    }

    /// Issue a JSON-RPC batch request. Every entry in `calls` becomes
    /// one element of a JSON array; the response array is parsed
    /// element-by-element and returned in the same order (the helper
    /// matches server-side `id` back to the request position, since
    /// the spec does not require in-order responses). See
    /// `plan/13-alchemy-adapter.md` §8.5.
    ///
    /// Outer `Result` is an all-or-nothing envelope error (network /
    /// transport / unparseable body); inner `Result<R, RpcError>` is
    /// the per-call status.
    ///
    /// Passing an empty `calls` slice returns `Ok(Vec::new())` without
    /// touching the network.
    pub async fn call_batch<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params_list: &[P],
    ) -> Result<Vec<Result<R, RpcError>>, RpcError> {
        if params_list.is_empty() {
            return Ok(Vec::new());
        }

        if let Some(breaker) = &self.breaker
            && breaker.is_open()
        {
            return Err(RpcError::CircuitOpen);
        }

        let mut request_array: Vec<Value> = Vec::with_capacity(params_list.len());
        for (idx, params) in params_list.iter().enumerate() {
            let params_value = serde_json::to_value(params)?;
            let id = u64::try_from(idx + 1).unwrap_or(u64::MAX);
            request_array.push(json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params_value,
            }));
        }
        let request = Value::Array(request_array);
        let expected = params_list.len();

        let outcome = retry_with_backoff(&self.retry, || async {
            let body = self.post_and_parse(&request).await?;
            parse_batch_envelope::<R>(body, expected)
        })
        .await;

        if let Some(breaker) = &self.breaker {
            match &outcome {
                Ok(_) => breaker.record_success(),
                Err(err) if is_breaker_failure(err) => breaker.record_failure(),
                Err(_) => {}
            }
        }
        if outcome.is_ok()
            && let Some(recorder) = &self.cost_recorder
        {
            let per_call = cost_hint_for(method).compute_units;
            let total = per_call.saturating_mul(u32::try_from(expected).unwrap_or(u32::MAX));
            recorder.record(total);
        }
        outcome
    }

    async fn post_and_parse(&self, request: &Value) -> Result<Value, RpcError> {
        let resp = self
            .http
            .post(self.base_url.clone())
            .json(request)
            .send()
            .await?;

        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(RpcError::Rate);
        }
        if resp.status().is_server_error() {
            return Err(RpcError::HttpServerError {
                status: resp.status().as_u16(),
            });
        }

        Ok(resp.json().await?)
    }
}

fn parse_single_rpc_envelope<R: DeserializeOwned>(body: Value) -> Result<R, RpcError> {
    if let Some(error) = body.get("error") {
        return Err(rpc_error_from_envelope(error));
    }

    let result = body.get("result").cloned().ok_or_else(|| RpcError::Rpc {
        code: -32000,
        message: "missing `result` field".to_string(),
    })?;

    serde_json::from_value(result).map_err(RpcError::Decode)
}

fn rpc_error_from_envelope(error: &Value) -> RpcError {
    let code = error.get("code").and_then(Value::as_i64).unwrap_or(-32000);
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unknown JSON-RPC error")
        .to_string();
    RpcError::Rpc { code, message }
}

fn parse_batch_envelope<R: DeserializeOwned>(
    body: Value,
    expected: usize,
) -> Result<Vec<Result<R, RpcError>>, RpcError> {
    let Value::Array(entries) = body else {
        return Err(RpcError::Rpc {
            code: -32000,
            message: "batch response is not a JSON array".to_string(),
        });
    };
    if entries.len() != expected {
        return Err(RpcError::Rpc {
            code: -32000,
            message: format!(
                "batch response size mismatch: got {}, expected {}",
                entries.len(),
                expected,
            ),
        });
    }

    // Build results placeholder; we match each entry to its `id`
    // slot (JSON-RPC 2.0 does not guarantee response ordering). If
    // any entry is missing a valid / unique id we fall back to
    // pairing by sequence — that matches the behaviour of every
    // server observed so far and keeps a well-formed batch parseable
    // even when the provider echoes `"id":null`.
    let mut out: Vec<Option<Result<R, RpcError>>> = (0..expected).map(|_| None).collect();
    let mut leftovers: Vec<Value> = Vec::new();
    for entry in entries {
        let slot = entry
            .get("id")
            .and_then(Value::as_u64)
            .and_then(|raw| usize::try_from(raw).ok())
            .and_then(|i| i.checked_sub(1))
            .filter(|i| *i < expected && out[*i].is_none());
        match slot {
            Some(idx) => out[idx] = Some(parse_single_rpc_envelope::<R>(entry)),
            None => leftovers.push(entry),
        }
    }
    for entry in leftovers {
        if let Some(idx) = out.iter().position(Option::is_none) {
            out[idx] = Some(parse_single_rpc_envelope::<R>(entry));
        }
    }

    out.into_iter()
        .map(|cell| {
            cell.ok_or_else(|| RpcError::Rpc {
                code: -32000,
                message: "batch response missed a slot".to_string(),
            })
        })
        .collect()
}

/// Parse a `0x`-prefixed hex string into `u128`. Used across the
/// adapters to decode JSON-RPC numerical responses.
pub fn parse_hex_u128(s: &str) -> Result<u128, RpcError> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    u128::from_str_radix(stripped, 16).map_err(|e| RpcError::Rpc {
        code: -32000,
        message: format!("invalid hex number: {e}"),
    })
}

/// Parse a `0x`-prefixed hex string into `u64`.
pub fn parse_hex_u64(s: &str) -> Result<u64, RpcError> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    u64::from_str_radix(stripped, 16).map_err(|e| RpcError::Rpc {
        code: -32000,
        message: format!("invalid hex number: {e}"),
    })
}
