//! Etherscan-backed [`LabelPort`].
//!
//! Reuses the V2 `contract/getsourcecode` endpoint: when the address
//! is a verified contract the response surfaces a `ContractName`,
//! which we promote to a [`Label`]. Unverified contracts and plain
//! EOAs return `Ok(None)`.
//!
//! See `plan/3-block-detail.md` §12.4.

use serde::Deserialize;

use super::client::{EtherscanClient, EtherscanError};
use crate::{
    application::ports::LabelPort,
    domain::{Address, Chain, DomainError, Label},
};

#[derive(Debug, Deserialize)]
struct GenericResponse {
    status: String,
    #[serde(default)]
    result: serde_json::Value,
    #[serde(default)]
    #[allow(dead_code)]
    message: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawEntry {
    #[serde(default)]
    contract_name: String,
}

#[derive(Debug, Clone)]
pub struct EtherscanLabel {
    client: EtherscanClient,
}

impl EtherscanLabel {
    #[must_use]
    pub fn new(client: EtherscanClient) -> Self {
        Self { client }
    }
}

impl LabelPort for EtherscanLabel {
    async fn label_for(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<Option<Label>, DomainError> {
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
        if parsed.status != "1" {
            // `getsourcecode` always returns status=1 for a
            // well-formed request. A 0 status on this endpoint
            // means the upstream is broken, not "no label" — but
            // we degrade to "no label" so the Overview row still
            // renders.
            return Ok(None);
        }
        let Some(array) = parsed.result.as_array() else {
            return Ok(None);
        };
        let Some(first) = array.first() else {
            return Ok(None);
        };
        let raw: RawEntry = serde_json::from_value(first.clone())
            .map_err(|e| DomainError::Internal(format!("etherscan label decode: {e}")))?;

        let name = raw.contract_name.trim();
        if name.is_empty() {
            return Ok(None);
        }
        Ok(Some(Label::etherscan(name)))
    }
}
