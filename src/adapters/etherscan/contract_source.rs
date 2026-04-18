//! Etherscan-backed [`ContractSourcePort`] returning the raw ABI
//! string for a verified contract.
//!
//! See `plan/4-tx-detail.md` section 12.4.1.

use serde::Deserialize;

use super::client::{EtherscanClient, EtherscanError};
use crate::{
    application::ports::ContractSourcePort,
    domain::{Address, Chain, ContractAbi, DomainError},
};

#[derive(Debug, Deserialize)]
struct GenericResponse {
    status: String,
    #[serde(default)]
    result: serde_json::Value,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Clone)]
pub struct EtherscanContractSource {
    client: EtherscanClient,
}

impl EtherscanContractSource {
    #[must_use]
    pub fn new(client: EtherscanClient) -> Self {
        Self { client }
    }

    /// Etherscan's convention for unverified contracts.
    const UNVERIFIED_MARKER: &'static str = "Contract source code not verified";
}

impl ContractSourcePort for EtherscanContractSource {
    async fn get_abi(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<Option<ContractAbi>, DomainError> {
        let addr_hex = address.to_hex();
        let response = self
            .client
            .get(
                chain,
                "contract",
                "getabi",
                &[("address", addr_hex.as_str())],
            )
            .await
            .map_err(EtherscanError::into_domain)?;

        let parsed: GenericResponse = serde_json::from_value(response.clone())
            .map_err(|e| DomainError::Internal(format!("etherscan decode: {e}")))?;

        // Etherscan convention: status "1" means OK, anything else
        // is an error string in `result`.
        let result_str = parsed
            .result
            .as_str()
            .ok_or_else(|| DomainError::Internal("etherscan result must be a string".into()))?;
        if parsed.status != "1" {
            // Unverified contracts surface as Ok(None) so the use
            // case can fall through to the signature directory.
            if result_str == Self::UNVERIFIED_MARKER
                || parsed.message.eq_ignore_ascii_case("NOTOK")
            {
                return Ok(None);
            }
            return Err(DomainError::Internal(format!(
                "etherscan error: {result_str}"
            )));
        }

        Ok(Some(ContractAbi {
            abi: result_str.to_string(),
            is_verified: true,
        }))
    }
}
