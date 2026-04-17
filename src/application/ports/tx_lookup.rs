//! Outbound port for looking up a transaction by hash.
//!
//! See `plan/2-search.md` section 4.

use crate::domain::{Chain, DomainError, TxHash, TxSummary};

pub trait TxLookupPort: Send + Sync {
    fn get(
        &self,
        hash: TxHash,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<TxSummary>, DomainError>> + Send;
}
