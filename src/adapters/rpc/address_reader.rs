//! Alchemy-backed [`AddressReaderPort`].
//!
//! Issues `eth_getBalance`, `eth_getTransactionCount` and `eth_getCode`
//! in parallel via `tokio::join!`, then assembles an
//! [`AddressOverview`]. Reverse-ENS lookup stays deferred; `ens_name`
//! is always `None` on the live path for now.
//!
//! See `plan/6-address-detail.md` section 12.2.

use super::client::{RpcClient, RpcError, parse_hex_u128, parse_hex_u64};
use crate::{
    application::ports::AddressReaderPort,
    domain::{Address, AddressKind, AddressOverview, Chain, DomainError, Wei},
};

#[derive(Debug, Clone)]
pub struct AlchemyAddressReader {
    client: RpcClient,
}

impl AlchemyAddressReader {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }
}

impl AddressReaderPort for AlchemyAddressReader {
    async fn get(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<Option<AddressOverview>, DomainError> {
        let hex = address.to_hex();

        let (balance_res, nonce_res, code_res): (
            Result<String, RpcError>,
            Result<String, RpcError>,
            Result<String, RpcError>,
        ) = tokio::join!(
            self.client.call("eth_getBalance", serde_json::json!([hex, "latest"])),
            self.client
                .call("eth_getTransactionCount", serde_json::json!([hex, "latest"])),
            self.client.call("eth_getCode", serde_json::json!([hex, "latest"])),
        );

        let balance_hex = balance_res.map_err(|e| e.into_domain())?;
        let nonce_hex = nonce_res.map_err(|e| e.into_domain())?;
        let code_hex = code_res.map_err(|e| e.into_domain())?;

        let balance = Wei::new(parse_hex_u128(&balance_hex).map_err(|e| e.into_domain())?);
        let nonce = parse_hex_u64(&nonce_hex).map_err(|e| e.into_domain())?;
        let kind = classify_code(&code_hex);

        Ok(Some(AddressOverview {
            chain,
            address,
            balance,
            nonce,
            kind,
            ens_name: None,
        }))
    }
}

fn classify_code(code: &str) -> AddressKind {
    let trimmed = code.strip_prefix("0x").unwrap_or(code);
    if trimmed.is_empty() || trimmed.chars().all(|c| c == '0') {
        AddressKind::Eoa
    } else {
        AddressKind::Contract
    }
}
