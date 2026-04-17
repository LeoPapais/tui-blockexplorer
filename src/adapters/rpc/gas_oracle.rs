//! Alchemy-backed implementation of
//! [`GasOraclePort`](crate::application::ports::GasOraclePort).
//!
//! Produces a [`GasSnapshot`] out of three JSON-RPC calls. See
//! `plan/13-alchemy-adapter.md` section 3.2.

use serde::Deserialize;

use super::client::{RpcClient, parse_hex_u128};
use crate::{
    application::ports::GasOraclePort,
    domain::{Chain, DomainError, GasSnapshot, Gwei},
};

#[derive(Debug, Deserialize)]
struct FeeHistoryResponse {
    #[serde(rename = "baseFeePerGas")]
    base_fee_per_gas: Vec<String>,
    reward: Option<Vec<Vec<String>>>,
}

/// Adapter implementing `GasOraclePort` over the Alchemy HTTP endpoint.
#[derive(Debug, Clone)]
pub struct AlchemyGasOracleAdapter {
    client: RpcClient,
}

impl AlchemyGasOracleAdapter {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }

    async fn fetch_snapshot(&self, chain: Chain) -> Result<GasSnapshot, DomainError> {
        // Kick off the three calls in parallel.
        let history_call = self.client.call::<_, FeeHistoryResponse>(
            "eth_feeHistory",
            serde_json::json!(["0x14", "latest", [25, 50, 75]]),
        );

        let history = history_call.await.map_err(|e| match e {
            // Some L2 chains expose the method but return an error;
            // translate the well-known "method not supported" case into
            // a domain-level "feature unavailable" so the screen can
            // show a friendly message.
            super::client::RpcError::Rpc { code: -32601, .. } => DomainError::FeatureUnavailable,
            other => other.into_domain(),
        })?;

        let base_fee_gwei = history
            .base_fee_per_gas
            .last()
            .map(|s| parse_hex_u128(s).map_err(|e| e.into_domain()))
            .transpose()?
            .map(|wei| Gwei::new(wei / 1_000_000_000))
            .unwrap_or_default();

        let trend = history
            .base_fee_per_gas
            .iter()
            .filter_map(|s| parse_hex_u128(s).ok())
            .map(|wei| Gwei::new(wei / 1_000_000_000))
            .collect::<Vec<_>>();

        let reward_rows = history.reward.unwrap_or_default();
        let (slow_tip, avg_tip, fast_tip) = average_reward_rows(&reward_rows)?;

        Ok(GasSnapshot {
            chain,
            slow: Gwei::new(base_fee_gwei.value() + slow_tip),
            average: Gwei::new(base_fee_gwei.value() + avg_tip),
            fast: Gwei::new(base_fee_gwei.value() + fast_tip),
            base_fee: base_fee_gwei,
            trend,
        })
    }
}

fn average_reward_rows(rows: &[Vec<String>]) -> Result<(u128, u128, u128), DomainError> {
    if rows.is_empty() {
        return Ok((0, 0, 0));
    }
    let mut totals = [0u128; 3];
    let mut counts = [0u128; 3];
    for row in rows {
        for (i, hex) in row.iter().take(3).enumerate() {
            let wei = parse_hex_u128(hex).map_err(|e| e.into_domain())?;
            totals[i] = totals[i].saturating_add(wei / 1_000_000_000);
            counts[i] += 1;
        }
    }
    let mean = |i: usize| totals[i].checked_div(counts[i]).unwrap_or(0);
    Ok((mean(0), mean(1), mean(2)))
}

impl GasOraclePort for AlchemyGasOracleAdapter {
    async fn snapshot(&self, chain: Chain) -> Result<GasSnapshot, DomainError> {
        self.fetch_snapshot(chain).await
    }
}
