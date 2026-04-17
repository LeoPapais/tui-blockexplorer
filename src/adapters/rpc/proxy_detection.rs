//! Alchemy-backed [`ProxyDetectionPort`].
//!
//! Reads the EIP-1967 implementation slot via `eth_getStorageAt` and
//! decodes the last 20 bytes into an `Address`. See
//! `plan/7-contract-detail.md` section 12.2.

use super::client::RpcClient;
use crate::{
    application::ports::ProxyDetectionPort,
    domain::{Address, Chain, DomainError, ProxyInfo, ProxyKind},
};

/// EIP-1967 implementation slot:
/// `keccak256("eip1967.proxy.implementation") - 1`.
const EIP1967_IMPL_SLOT: &str =
    "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc";

#[derive(Debug, Clone)]
pub struct AlchemyProxyDetector {
    client: RpcClient,
}

impl AlchemyProxyDetector {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }
}

impl ProxyDetectionPort for AlchemyProxyDetector {
    async fn detect(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<Option<ProxyInfo>, DomainError> {
        let slot_value: String = self
            .client
            .call(
                "eth_getStorageAt",
                serde_json::json!([address.to_hex(), EIP1967_IMPL_SLOT, "latest"]),
            )
            .await
            .map_err(|e| e.into_domain())?;

        match decode_implementation(&slot_value)? {
            Some(impl_address) => Ok(Some(ProxyInfo {
                kind: ProxyKind::Eip1967,
                implementation: impl_address,
            })),
            None => Ok(None),
        }
    }
}

fn decode_implementation(slot: &str) -> Result<Option<Address>, DomainError> {
    let stripped = slot.strip_prefix("0x").unwrap_or(slot);
    if stripped.len() != 64 {
        return Err(DomainError::Internal(format!(
            "expected 32-byte slot value, got {}",
            stripped.len()
        )));
    }
    // All zeros means no proxy.
    if stripped.chars().all(|c| c == '0') {
        return Ok(None);
    }
    // Last 20 bytes (40 hex chars) contain the address.
    let tail = &stripped[stripped.len() - 40..];
    let formatted = format!("0x{tail}");
    Ok(Some(Address::from_hex(&formatted)?))
}
