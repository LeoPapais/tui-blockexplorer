//! Etherscan V2 health probe. Calls
//! `module=stats&action=chainsize` which every V2 endpoint advertises.
//!
//! See `plan/10-settings.md` section 12.5.

use std::time::Instant;

use super::client::{EtherscanClient, EtherscanError};
use crate::{
    application::ports::HealthStatus,
    domain::{Chain, DomainError},
};

/// Provider label used by [`EtherscanHealth`] and in the Settings UI.
pub const ETHERSCAN_PROVIDER: &str = "etherscan-v2";

/// Adapter: probes the Etherscan V2 endpoint via `stats/chainsize`.
#[derive(Debug, Clone)]
pub struct EtherscanHealth {
    client: EtherscanClient,
}

impl EtherscanHealth {
    #[must_use]
    pub fn new(client: EtherscanClient) -> Self {
        Self { client }
    }

    pub async fn probe(&self, chain: Chain) -> Result<HealthStatus, DomainError> {
        let start = Instant::now();
        // `chainsize` requires a start / end date; we pass a wide
        // enough window that the endpoint always has something to
        // return. The latency matters more than the payload.
        let result = self
            .client
            .get(
                chain,
                "stats",
                "chainsize",
                &[
                    ("startdate", "2020-01-01"),
                    ("enddate", "2020-01-02"),
                    ("clienttype", "geth"),
                    ("syncmode", "default"),
                ],
            )
            .await;
        let latency_ms = start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

        match result {
            Ok(body) => {
                match body.get("status").and_then(|v| v.as_str()) {
                    Some("1") => Ok(HealthStatus::healthy(ETHERSCAN_PROVIDER, latency_ms)),
                    Some("0") => {
                        let msg = body
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("status=0")
                            .to_string();
                        Ok(HealthStatus::degraded(ETHERSCAN_PROVIDER, latency_ms, msg))
                    }
                    other => Ok(HealthStatus::degraded(
                        ETHERSCAN_PROVIDER,
                        latency_ms,
                        format!("unexpected status field: {other:?}"),
                    )),
                }
            }
            Err(EtherscanError::Http(err)) if err.is_timeout() || err.is_connect() => Ok(
                HealthStatus::down(ETHERSCAN_PROVIDER, latency_ms, err.to_string()),
            ),
            Err(err) => Ok(HealthStatus::down(
                ETHERSCAN_PROVIDER,
                latency_ms,
                err.to_string(),
            )),
        }
    }
}
