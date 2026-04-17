//! Outbound port for classifying an address as EOA or contract.
//!
//! See `plan/2-search.md` section 4.

use crate::domain::{Address, AddressKind, Chain, DomainError};

pub trait AddressLookupPort: Send + Sync {
    fn classify(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<AddressKind, DomainError>> + Send;
}
