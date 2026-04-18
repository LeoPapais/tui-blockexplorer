//! Alchemy-backed [`ProxyDetectionPort`].
//!
//! Probes the EIP-1967-family storage slots via `eth_getStorageAt`
//! and returns the first non-zero hit as a [`ProxyInfo`]. See
//! `plan/7-contract-detail.md` sections 12.2 (MVP EIP-1967 read)
//! and 12.5.2 (UUPS + Transparent slot probing).

use super::client::RpcClient;
use crate::{
    application::ports::ProxyDetectionPort,
    domain::{Address, Chain, DomainError, ProxyInfo, ProxySource},
};

/// EIP-1967 implementation slot:
/// `keccak256("eip1967.proxy.implementation") - 1`.
const EIP1967_IMPL_SLOT: &str =
    "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc";

/// EIP-1822 UUPS `PROXIABLE` slot:
/// `keccak256("PROXIABLE")`. Populated inside the implementation
/// itself; when the proxy delegates to it, reading the slot through
/// the proxy returns the implementation address.
const EIP1822_PROXIABLE_SLOT: &str =
    "0xc5f16f0fcc639fa48a6947836d9850f504798523bf8c9a3a87d5876cf622bcf7";

/// OpenZeppelin Transparent proxy admin slot:
/// `keccak256("eip1967.proxy.admin") - 1`. The value stored there is
/// the admin address, not the implementation; plan/7 §13 tracks a
/// follow-up that reads both impl and admin slots and renders them
/// as two rows.
const TRANSPARENT_ADMIN_SLOT: &str =
    "0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103";

#[derive(Debug, Clone)]
pub struct AlchemyProxyDetector {
    client: RpcClient,
}

impl AlchemyProxyDetector {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }

    async fn read_slot(
        &self,
        address: Address,
        slot: &str,
    ) -> Result<Option<Address>, DomainError> {
        let slot_value: String = self
            .client
            .call(
                "eth_getStorageAt",
                serde_json::json!([address.to_hex(), slot, "latest"]),
            )
            .await
            .map_err(|e| e.into_domain())?;
        decode_implementation(&slot_value)
    }
}

impl ProxyDetectionPort for AlchemyProxyDetector {
    async fn detect(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<Option<ProxyInfo>, DomainError> {
        // Probe the three slots in order. EIP-1967 first because it
        // is the most common by an order of magnitude, UUPS second,
        // Transparent admin slot last (also rare on its own).
        for (slot, source) in [
            (EIP1967_IMPL_SLOT, ProxySource::Eip1967Slot),
            (EIP1822_PROXIABLE_SLOT, ProxySource::Eip1822Slot),
            (TRANSPARENT_ADMIN_SLOT, ProxySource::TransparentSlot),
        ] {
            if let Some(impl_address) = self.read_slot(address, slot).await? {
                let info = match source {
                    ProxySource::Eip1967Slot => ProxyInfo::eip1967_slot(impl_address),
                    ProxySource::Eip1822Slot => ProxyInfo::uups_slot(impl_address),
                    ProxySource::TransparentSlot => ProxyInfo::transparent_admin_slot(impl_address),
                    // Unreachable: only the three slot-based
                    // sources are probed here. The Etherscan hint
                    // runs one layer up in the composite detector.
                    ProxySource::EtherscanHint => ProxyInfo::etherscan_hint(impl_address),
                };
                return Ok(Some(info));
            }
        }
        Ok(None)
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
    if stripped.chars().all(|c| c == '0') {
        return Ok(None);
    }
    let tail = &stripped[stripped.len() - 40..];
    let formatted = format!("0x{tail}");
    Ok(Some(Address::from_hex(&formatted)?))
}
