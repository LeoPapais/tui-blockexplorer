//! Outbound port that feeds the Mempool screen with pending-tx deltas.
//!
//! See `plan/5-mempool.md` section 11.1 (subscribe) and §11.3.2
//! (update_filter).

use tokio::sync::mpsc::UnboundedReceiver;

use crate::domain::{Chain, DomainError, PendingTxEvent, PendingTxFilter};

pub trait PendingTxStreamPort: Send + Sync {
    /// Open a subscription on the given chain with the given filter.
    /// Returns the receiving half of a channel that will keep
    /// producing [`PendingTxEvent`] values until dropped.
    fn subscribe(
        &self,
        chain: Chain,
        filter: PendingTxFilter,
    ) -> impl std::future::Future<Output = Result<UnboundedReceiver<PendingTxEvent>, DomainError>> + Send;

    /// Forward a filter change to the upstream provider. Providers
    /// that support server-side filtering (Alchemy
    /// `alchemy_pendingTransactions`) re-negotiate the subscription;
    /// providers that do not can record the filter for diagnostics
    /// and return `Ok(())`. The UI still applies the filter
    /// client-side, so this call is best-effort.
    ///
    /// See `plan/5-mempool.md` §11.3.2.
    fn update_filter(
        &self,
        chain: Chain,
        filter: PendingTxFilter,
    ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send;
}
