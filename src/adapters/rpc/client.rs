//! Generic JSON-RPC 2.0 client for Alchemy HTTP endpoints.
//!
//! See `plan/13-alchemy-adapter.md` section 2.

use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use thiserror::Error;
use url::Url;

use crate::domain::DomainError;

/// Errors produced by the RPC client. Mapped to [`DomainError`] at the
/// adapter boundary via [`RpcError::into_domain`].
#[derive(Debug, Error)]
pub enum RpcError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON-RPC error {code}: {message}")]
    Rpc { code: i64, message: String },

    #[error("response could not be decoded: {0}")]
    Decode(#[from] serde_json::Error),

    #[error("rate limited")]
    Rate,

    #[error("request timed out")]
    Timeout,
}

impl RpcError {
    pub fn into_domain(self) -> DomainError {
        match self {
            RpcError::Rate | RpcError::Timeout => DomainError::ProviderUnavailable,
            RpcError::Http(err) if err.is_timeout() => DomainError::ProviderUnavailable,
            RpcError::Http(err) if err.is_connect() => DomainError::ProviderUnavailable,
            RpcError::Http(err) => DomainError::Internal(err.to_string()),
            RpcError::Rpc { code: -32005, .. } => DomainError::ProviderUnavailable,
            RpcError::Rpc { message, .. } => DomainError::Internal(message),
            RpcError::Decode(err) => DomainError::Internal(err.to_string()),
        }
    }
}

/// Thin JSON-RPC 2.0 client. One instance is reused across calls on the
/// same base URL.
#[derive(Debug, Clone)]
pub struct RpcClient {
    http: Client,
    base_url: Url,
}

impl RpcClient {
    /// Construct a new client. Callers build the `reqwest::Client` once
    /// and pass it in so connection pooling is shared across adapters.
    #[must_use]
    pub fn new(base_url: Url, http: Client) -> Self {
        Self { http, base_url }
    }

    /// Convenience constructor: builds a `reqwest::Client` with sane
    /// defaults (10 second timeout, rustls-only TLS).
    pub fn with_default_http(base_url: Url) -> Result<Self, RpcError> {
        let http = Client::builder().timeout(Duration::from_secs(10)).build()?;
        Ok(Self::new(base_url, http))
    }

    /// Issue a JSON-RPC 2.0 call and decode the `result` field.
    pub async fn call<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R, RpcError> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let resp = self
            .http
            .post(self.base_url.clone())
            .json(&request)
            .send()
            .await?;

        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(RpcError::Rate);
        }
        if resp.status().is_server_error() {
            return Err(RpcError::Rate);
        }

        let body: Value = resp.json().await?;

        if let Some(error) = body.get("error") {
            let code = error.get("code").and_then(Value::as_i64).unwrap_or(-32000);
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown JSON-RPC error")
                .to_string();
            return Err(RpcError::Rpc { code, message });
        }

        let result = body.get("result").cloned().ok_or_else(|| RpcError::Rpc {
            code: -32000,
            message: "missing `result` field".to_string(),
        })?;

        serde_json::from_value(result).map_err(RpcError::Decode)
    }
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
