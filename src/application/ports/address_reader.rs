//! Outbound port for the Address Detail Overview tab.
//!
//! See `plan/6-address-detail.md` section 12.1.

use crate::domain::{Address, AddressOverview, Chain, DomainError};

pub trait AddressReaderPort: Send + Sync {
    fn get(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<AddressOverview>, DomainError>> + Send;
}
