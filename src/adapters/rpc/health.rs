//! Alchemy health probe. Calls `eth_chainId` and compares the
//! returned chain id against the one expected by `chain`.
//!
//! See `plan/10-settings.md` section 12.5.

use std::time::Instant;

use super::client::{RpcClient, parse_hex_u64};
use crate::{
    application::ports::HealthStatus,
    domain::{Chain, DomainError},
};

/// Provider label used by [`AlchemyHealth`] and in the Settings UI.
pub const ALCHEMY_PROVIDER: &str = "alchemy";

/// Adapter: probes the Alchemy JSON-RPC endpoint via `eth_chainId`.
#[derive(Debug, Clone)]
pub struct AlchemyHealth {
    client: RpcClient,
}

impl AlchemyHealth {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }

    pub async fn probe(&self, chain: Chain) -> Result<HealthStatus, DomainError> {
        let start = Instant::now();
        let result: Result<String, _> =
            self.client.call("eth_chainId", serde_json::json!([])).await;
        let latency_ms = start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

        match result {
            Ok(hex) => {
                let id = parse_hex_u64(&hex).map_err(|e| DomainError::Internal(e.to_string()))?;
                if id == expected_chain_id(chain) {
                    Ok(HealthStatus::healthy(ALCHEMY_PROVIDER, latency_ms))
                } else {
                    Ok(HealthStatus::degraded(
                        ALCHEMY_PROVIDER,
                        latency_ms,
                        format!(
                            "expected chain id {expected}, got {id}",
                            expected = expected_chain_id(chain),
                            id = id,
                        ),
                    ))
                }
            }
            Err(err) => Ok(HealthStatus::down(
                ALCHEMY_PROVIDER,
                latency_ms,
                err.to_string(),
            )),
        }
    }
}

/// Numeric chain id expected from the JSON-RPC endpoint for `chain`.
#[must_use]
pub const fn expected_chain_id(chain: Chain) -> u64 {
    match chain {
        Chain::Ethereum => 1,
        Chain::EthereumSepolia => 11_155_111,
        Chain::Base => 8_453,
        Chain::Polygon => 137,
        Chain::Optimism => 10,
        Chain::Arbitrum => 42_161,
    }
}
