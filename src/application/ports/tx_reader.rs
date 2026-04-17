//! Outbound port for fetching the full `Transaction` entity shown on
//! the TxDetail screen.
//!
//! See `plan/4-tx-detail.md` section 12.1.

use crate::domain::{Chain, DomainError, Transaction, TxHash};

pub trait TxReaderPort: Send + Sync {
    /// Fetch the full transaction identified by `hash`. Returns
    /// `Ok(None)` when it does not exist; surfaces provider failures
    /// through `Err(DomainError::*)`.
    fn get(
        &self,
        hash: TxHash,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<Transaction>, DomainError>> + Send;
}
