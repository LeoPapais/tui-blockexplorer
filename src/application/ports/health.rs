//! Health port.
//!
//! Exposes per-provider health checks that the Settings screen
//! surfaces to the user. Adapters ping a cheap endpoint
//! (Alchemy `eth_chainId`, Etherscan `stats/chainsize`) and map the
//! outcome into the domain status below.
//!
//! See `plan/10-settings.md` section 12.5.

use std::future::Future;

use crate::domain::{Chain, DomainError};

/// Semantic health level returned by the provider ping. Adapters
/// distinguish between a healthy response, a successful response
/// that semantically signals trouble (429, rate-limit advisory,
/// chain mismatch), and an outright failure (connection refused,
/// timeout).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthLevel {
    Healthy,
    Degraded,
    Down,
}

/// Provider-scoped health snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthStatus {
    pub provider: &'static str,
    pub status: HealthLevel,
    /// Round-trip time of the probe call, in milliseconds.
    pub latency_ms: u64,
    /// Optional diagnostic message surfaced by the adapter.
    pub message: Option<String>,
}

impl HealthStatus {
    /// Shorthand builder for the healthy path. Keeps adapter code
    /// concise.
    #[must_use]
    pub fn healthy(provider: &'static str, latency_ms: u64) -> Self {
        Self {
            provider,
            status: HealthLevel::Healthy,
            latency_ms,
            message: None,
        }
    }

    /// Shorthand builder for a degraded-but-alive response.
    #[must_use]
    pub fn degraded(provider: &'static str, latency_ms: u64, msg: impl Into<String>) -> Self {
        Self {
            provider,
            status: HealthLevel::Degraded,
            latency_ms,
            message: Some(msg.into()),
        }
    }

    /// Shorthand builder for a confirmed outage.
    #[must_use]
    pub fn down(provider: &'static str, latency_ms: u64, msg: impl Into<String>) -> Self {
        Self {
            provider,
            status: HealthLevel::Down,
            latency_ms,
            message: Some(msg.into()),
        }
    }
}

/// Outbound port that every health-probing adapter implements.
pub trait HealthPort: Send + Sync {
    /// Probe the Alchemy JSON-RPC endpoint for `chain`. Adapters
    /// issue a single cheap call (typically `eth_chainId`).
    fn alchemy(
        &self,
        chain: Chain,
    ) -> impl Future<Output = Result<HealthStatus, DomainError>> + Send;

    /// Probe the Etherscan V2 endpoint for `chain`. Adapters issue a
    /// lightweight request (`module=stats&action=chainsize`).
    fn etherscan(
        &self,
        chain: Chain,
    ) -> impl Future<Output = Result<HealthStatus, DomainError>> + Send;
}
