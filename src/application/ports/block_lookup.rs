//! Outbound port for looking blocks up by hash or number.
//!
//! See `plan/2-search.md` section 4.

use crate::domain::{BlockHash, BlockNumber, BlockSummary, Chain, DomainError};

pub trait BlockLookupPort: Send + Sync {
    fn get_by_hash(
        &self,
        hash: BlockHash,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<BlockSummary>, DomainError>> + Send;

    fn get_by_number(
        &self,
        number: BlockNumber,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<BlockSummary>, DomainError>> + Send;
}
