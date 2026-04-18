//! Alchemy-backed implementation of
//! [`NetworkStatusPort`](crate::application::ports::NetworkStatusPort).
//!
//! Pulls the head block via `eth_blockNumber`, then reads the full
//! block to derive the base fee and block-time estimate. See
//! `plan/13-alchemy-adapter.md` section 3.1.

use serde::Deserialize;

use super::client::{RpcClient, parse_hex_u64, parse_hex_u128};
use crate::{
    application::ports::NetworkStatusPort,
    domain::{BlockNumber, Chain, DomainError, NetworkStatus, Wei},
};

#[derive(Debug, Deserialize)]
struct BlockResponse {
    number: String,
    #[serde(rename = "baseFeePerGas", default)]
    base_fee_per_gas: Option<String>,
    timestamp: String,
    #[serde(rename = "parentHash")]
    parent_hash: String,
}

#[derive(Debug, Deserialize)]
struct ParentBlockResponse {
    timestamp: String,
}

/// Adapter implementing `NetworkStatusPort` over the Alchemy HTTP
/// endpoint.
#[derive(Debug, Clone)]
pub struct AlchemyNetworkStatusAdapter {
    client: RpcClient,
}

impl AlchemyNetworkStatusAdapter {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }

    async fn fetch_snapshot(&self, chain: Chain) -> Result<NetworkStatus, DomainError> {
        // Head block number.
        let head_hex: String = self
            .client
            .call("eth_blockNumber", serde_json::json!([]))
            .await
            .map_err(|e| e.into_domain())?;
        let head_number = parse_hex_u64(&head_hex).map_err(|e| e.into_domain())?;

        // Head block itself (full = false; only hashes).
        let head: BlockResponse = self
            .client
            .call(
                "eth_getBlockByNumber",
                serde_json::json!([format!("0x{head_number:x}"), false]),
            )
            .await
            .map_err(|e| e.into_domain())?;

        let base_fee = match head.base_fee_per_gas.as_deref() {
            Some(hex) => Wei::new(parse_hex_u128(hex).map_err(|e| e.into_domain())?),
            None => Wei::new(0),
        };

        // Parent block for a rough block-time estimate. Best effort:
        // failure is non-fatal since the home screen can survive without
        // it.
        let block_time_avg_ms = match self
            .client
            .call::<_, ParentBlockResponse>(
                "eth_getBlockByHash",
                serde_json::json!([head.parent_hash, false]),
            )
            .await
        {
            Ok(parent) => {
                let head_ts = parse_hex_u64(&head.timestamp).unwrap_or(0);
                let parent_ts = parse_hex_u64(&parent.timestamp).unwrap_or(head_ts);
                head_ts.saturating_sub(parent_ts).saturating_mul(1000)
            }
            Err(_) => 0,
        };

        let head_number = parse_hex_u64(&head.number).map_err(|e| e.into_domain())?;

        Ok(NetworkStatus {
            chain,
            latest_block: BlockNumber::new(head_number),
            base_fee,
            block_time_avg_ms,
        })
    }
}

impl NetworkStatusPort for AlchemyNetworkStatusAdapter {
    async fn snapshot(&self, chain: Chain) -> Result<NetworkStatus, DomainError> {
        self.fetch_snapshot(chain).await
    }
}
