//! Etherscan-backed [`EtherscanProxyHintPort`].
//!
//! Reads `contract/getsourcecode` and extracts the `Implementation`
//! field — Etherscan populates it whenever its own proxy flag is set,
//! even for contracts (EIP-1167, diamonds, older custom proxies) that
//! leave the EIP-1967 implementation slot zero. The hint is
//! pessimistic: if the field is empty, the zero address, or
//! `"0x"`, we return `None` and let the caller fall through.
//!
//! See `plan/7-contract-detail.md` section 12.5.1.

use serde::Deserialize;

use super::client::{EtherscanClient, EtherscanError};
use crate::{
    application::ports::EtherscanProxyHintPort,
    domain::{Address, Chain, DomainError},
};

#[derive(Debug, Deserialize)]
struct GenericResponse {
    status: String,
    #[serde(default)]
    result: serde_json::Value,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct HintEntry {
    #[serde(default)]
    implementation: String,
}

#[derive(Debug, Clone)]
pub struct EtherscanProxyHint {
    client: EtherscanClient,
}

impl EtherscanProxyHint {
    #[must_use]
    pub fn new(client: EtherscanClient) -> Self {
        Self { client }
    }
}

impl EtherscanProxyHintPort for EtherscanProxyHint {
    async fn implementation_hint(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<Option<Address>, DomainError> {
        let addr_hex = address.to_hex();
        let response = self
            .client
            .get(
                chain,
                "contract",
                "getsourcecode",
                &[("address", addr_hex.as_str())],
            )
            .await
            .map_err(EtherscanError::into_domain)?;

        let parsed: GenericResponse = serde_json::from_value(response)
            .map_err(|e| DomainError::Internal(format!("etherscan decode: {e}")))?;

        // `status != "1"` on `getsourcecode` almost always means
        // "no key" / "rate limited" / "provider unavailable"; keep
        // the caller informed rather than pretending there is no
        // hint, mirroring `EtherscanContractSource::get_source`.
        if parsed.status != "1" {
            let hint = parsed.result.as_str().unwrap_or(&parsed.message);
            return Err(DomainError::Internal(format!("etherscan error: {hint}")));
        }

        let Some(array) = parsed.result.as_array() else {
            return Ok(None);
        };
        let Some(first) = array.first() else {
            return Ok(None);
        };
        let entry: HintEntry = serde_json::from_value(first.clone())
            .map_err(|e| DomainError::Internal(format!("etherscan hint decode: {e}")))?;

        let trimmed = entry.implementation.trim();
        if trimmed.is_empty()
            || trimmed == "0x"
            || trimmed == "0x0000000000000000000000000000000000000000"
        {
            return Ok(None);
        }
        match Address::from_hex(trimmed) {
            Ok(addr) => Ok(Some(addr)),
            // A malformed address is a provider bug, not a real
            // "no hint" — surface it so tests fail loudly rather
            // than silently missing a fallback.
            Err(e) => Err(DomainError::Internal(format!(
                "etherscan returned malformed implementation address: {e}"
            ))),
        }
    }
}
