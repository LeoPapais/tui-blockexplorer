//! Use case: load the Address Detail Overview view-model.
//!
//! See `plan/6-address-detail.md` section 12.1.

use crate::{
    application::ports::AddressReaderPort,
    domain::{Address, AddressOverview, Chain, DomainError},
};

pub async fn run<P: AddressReaderPort>(
    reader: &P,
    address: Address,
    chain: Chain,
) -> Result<AddressOverview, DomainError> {
    match reader.get(address, chain).await? {
        Some(overview) => Ok(overview),
        None => Err(DomainError::NotFound),
    }
}
