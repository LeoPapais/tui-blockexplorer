//! Outbound port returning the unified transfers feed for an address.
//!
//! Backed by `alchemy_getAssetTransfers` in live mode. The adapter
//! issues two parallel queries (fromAddress / toAddress) and merges
//! the results by block number descending.
//!
//! See `plan/6-address-detail.md` section 12.4.1.

use crate::domain::{Address, Chain, DomainError, TransferCursor, TransferPage};

pub trait TransfersPort: Send + Sync {
    /// Fetch a page of transfers for `address` on `chain`.
    /// `cursor` is `None` for the first page, or the `next_cursor`
    /// from a previous call.
    fn get_for_address(
        &self,
        address: Address,
        chain: Chain,
        cursor: Option<TransferCursor>,
    ) -> impl std::future::Future<Output = Result<TransferPage, DomainError>> + Send;

    /// Fetch a page of transfers whose asset is emitted by
    /// `contract`. Maps to an `alchemy_getAssetTransfers` query with
    /// `contractAddresses=[contract]` and `category=["erc20"]`.
    ///
    /// Used by the Token Detail "Transfers" tab (see
    /// `plan/8-token-detail.md` section 12.4).
    fn get_for_contract(
        &self,
        contract: Address,
        chain: Chain,
        cursor: Option<TransferCursor>,
    ) -> impl std::future::Future<Output = Result<TransferPage, DomainError>> + Send;
}
