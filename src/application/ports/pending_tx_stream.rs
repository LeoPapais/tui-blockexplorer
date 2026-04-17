//! Outbound port that feeds the Mempool screen with pending-tx deltas.
//!
//! See `plan/5-mempool.md` section 11.1.

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
    ) -> impl std::future::Future<
        Output = Result<UnboundedReceiver<PendingTxEvent>, DomainError>,
    > + Send;
}
