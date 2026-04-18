//! Outbound port for the Transactions tab on the Block Detail screen.
//!
//! Returns every transaction in a block alongside its receipt data in
//! a single call. The adapter typically issues
//! `eth_getBlockByNumber` (full transactions) and
//! `eth_getBlockReceipts` in parallel and zips the two by hash.
//!
//! See `plan/3-block-detail.md` §12.3.

use crate::domain::{BlockId, BlockTxReceipt, Chain, DomainError};

pub trait BlockReceiptsPort: Send + Sync {
    /// Fetch every transaction in the block plus its receipt fields.
    /// Rows are returned in block / tx-index order.
    ///
    /// Missing blocks surface as `Err(DomainError::NotFound)` so the
    /// use case does not have to distinguish "block missing" from
    /// "block has zero txs" (the latter returns an empty vec).
    fn get_transactions(
        &self,
        id: BlockId,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Vec<BlockTxReceipt>, DomainError>> + Send;
}
