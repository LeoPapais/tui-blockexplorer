//! Alchemy-backed [`StoragePort`] adapter.
//!
//! Wraps `eth_getStorageAt` at tag `"latest"`. See
//! `plan/7-contract-detail.md` section 12.4.3.

use serde_json::json;

use super::client::{RpcClient, RpcError};
use crate::{
    application::ports::StoragePort,
    domain::{Address, Chain, DomainError},
};

#[derive(Debug, Clone)]
pub struct AlchemyStorage {
    client: RpcClient,
}

impl AlchemyStorage {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }
}

impl StoragePort for AlchemyStorage {
    async fn get_at(
        &self,
        address: Address,
        _chain: Chain,
        slot: [u8; 32],
    ) -> Result<[u8; 32], DomainError> {
        let slot_hex = format!("0x{}", hex::encode(slot));
        let raw: String = self
            .client
            .call(
                "eth_getStorageAt",
                json!([address.to_hex(), slot_hex, "latest"]),
            )
            .await
            .map_err(|e: RpcError| match e {
                RpcError::Rpc { code: -32601, .. } => DomainError::FeatureUnavailable,
                other => other.into_domain(),
            })?;
        decode_word(&raw)
    }
}

fn decode_word(s: &str) -> Result<[u8; 32], DomainError> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    if stripped.len() != 64 {
        return Err(DomainError::Internal(format!(
            "eth_getStorageAt returned {} hex chars, expected 64",
            stripped.len()
        )));
    }
    let mut out = [0u8; 32];
    hex::decode_to_slice(stripped, &mut out)
        .map_err(|e| DomainError::Internal(format!("invalid hex: {e}")))?;
    Ok(out)
}
