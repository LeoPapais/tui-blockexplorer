//! Outbound port reading a raw storage slot from a contract.
//! Backed by `eth_getStorageAt` in live mode.
//!
//! See `plan/7-contract-detail.md` section 12.4.3.

use crate::domain::{Address, Chain, DomainError};

pub trait StoragePort: Send + Sync {
    fn get_at(
        &self,
        address: Address,
        chain: Chain,
        slot: [u8; 32],
    ) -> impl std::future::Future<Output = Result<[u8; 32], DomainError>> + Send;
}
