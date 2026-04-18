//! Alchemy-backed [`AddressLookupPort`].
//!
//! Classifies an address via `eth_getCode` and delegates the hex
//! parsing to [`classify_address`] so the EIP-7702 delegation tag is
//! peeled off uniformly with the address-reader adapter.
//!
//! See `plan/2-search.md` section 10.2 and `plan/15-backlog.md`
//! section 3.1.

use super::client::RpcClient;
use crate::{
    application::{ports::AddressLookupPort, use_cases::classify_address},
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

        classify_address::run(&code)
    }
}
