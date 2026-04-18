//! Etherscan-backed [`ContractSourcePort`] returning the raw ABI
//! string plus (via `get_source`) the full verified source and
//! compiler metadata.
//!
//! See `plan/4-tx-detail.md` section 12.4.1 and
//! `plan/7-contract-detail.md` section 12.4.1.

use serde::Deserialize;

use super::client::{EtherscanClient, EtherscanError};
use crate::{
    application::ports::ContractSourcePort,
    domain::{
        Address, Chain, ContractAbi, ContractSource, DomainError,
        contract_source::parse_etherscan_source_envelope,
    },
};

#[derive(Debug, Deserialize)]
struct GenericResponse {
    status: String,
    #[serde(default)]
    result: serde_json::Value,
    #[serde(default)]
    message: String,
}

/// Shape of a single entry inside `getsourcecode`'s `result` array.
/// Every field is optional because Etherscan occasionally omits or
/// empties them.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawSourceEntry {
    #[serde(default)]
    source_code: String,
    #[serde(default, rename = "ABI")]
    abi: String,
    #[serde(default)]
    contract_name: String,
    #[serde(default)]
    compiler_version: String,
    #[serde(default)]
    optimization_used: String,
    #[serde(default)]
    runs: String,
    #[serde(default, rename = "EVMVersion")]
    evm_version: String,
    #[serde(default)]
    license_type: String,
    #[serde(default)]
    #[allow(dead_code)]
    proxy: String,
    #[serde(default)]
    implementation: String,
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

        let parsed: GenericResponse = serde_json::from_value(response)
            .map_err(|e| DomainError::Internal(format!("etherscan decode: {e}")))?;

        let result_str = parsed
            .result
            .as_str()
            .ok_or_else(|| DomainError::Internal("etherscan result must be a string".into()))?;
        if parsed.status != "1" {
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

    async fn get_source(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<Option<ContractSource>, DomainError> {
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

        // getsourcecode always returns an array even when the
        // contract is unverified. Status == "1" just means "request
        // succeeded"; we still have to inspect the row.
        let parsed: GenericResponse = serde_json::from_value(response)
            .map_err(|e| DomainError::Internal(format!("etherscan decode: {e}")))?;
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
        let raw: RawSourceEntry = serde_json::from_value(first.clone())
            .map_err(|e| DomainError::Internal(format!("etherscan source decode: {e}")))?;

        // Unverified -> SourceCode is empty and ABI is the marker.
        if raw.source_code.trim().is_empty() || raw.abi.contains(Self::UNVERIFIED_MARKER) {
            return Ok(None);
        }

        let fallback_name = if raw.contract_name.is_empty() {
            "Contract.sol".to_string()
        } else {
            format!("{}.sol", raw.contract_name)
        };
        let files = parse_etherscan_source_envelope(&raw.source_code, &fallback_name);

        let implementation = if raw.implementation.is_empty()
            || raw.implementation == "0x"
            || raw.implementation
                == "0x0000000000000000000000000000000000000000"
        {
            None
        } else {
            Address::from_hex(&raw.implementation).ok()
        };

        Ok(Some(ContractSource {
            is_verified: true,
            contract_name: raw.contract_name,
            compiler_version: raw.compiler_version,
            optimizer_enabled: raw.optimization_used.trim() == "1",
            optimizer_runs: raw.runs.parse().unwrap_or(0),
            evm_version: raw.evm_version,
            license: raw.license_type,
            abi: raw.abi,
            files,
            implementation,
        }))
    }
}
