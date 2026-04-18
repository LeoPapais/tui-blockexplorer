//! Alchemy-backed [`AddressLookupPort`].
//!
//! Classifies an address via `eth_getCode`: empty code (`0x`) means EOA;
//! anything else means contract. See `plan/2-search.md` section 10.2.

use super::client::RpcClient;
use crate::{
    application::ports::AddressLookupPort,
    domain::{Address, AddressKind, Chain, DomainError},
};

#[derive(Debug, Clone)]
pub struct AlchemyAddressLookup {
    client: RpcClient,
}

impl AlchemyAddressLookup {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }
}

impl AddressLookupPort for AlchemyAddressLookup {
    async fn classify(&self, address: Address, _chain: Chain) -> Result<AddressKind, DomainError> {
        let code: String = self
            .client
            .call(
                "eth_getCode",
                serde_json::json!([address.to_hex(), "latest"]),
            )
            .await
            .map_err(|e| e.into_domain())?;

        // `0x` (no code) -> EOA; anything longer means bytecode is
        // deployed -> contract.
        let trimmed = code.strip_prefix("0x").unwrap_or(&code);
        if trimmed.is_empty() || trimmed.chars().all(|c| c == '0') {
            Ok(AddressKind::Eoa)
        } else {
            Ok(AddressKind::Contract)
        }
    }
}
