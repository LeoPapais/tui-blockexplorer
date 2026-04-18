//! Use case that opens a pending-tx subscription through the port and
//! returns the resulting receiver to the caller. Also exposes the thin
//! wrapper around `PendingTxStreamPort::update_filter` so the screen
//! layer never depends on the port type directly.
//!
//! See `plan/5-mempool.md` §11.1 (subscribe) and §11.3.2
//! (update_filter).

use tokio::sync::mpsc::UnboundedReceiver;

use crate::{
    application::ports::PendingTxStreamPort,
    domain::{Chain, DomainError, PendingTxEvent, PendingTxFilter},
};

pub async fn run<P: PendingTxStreamPort>(
    port: &P,
    chain: Chain,
    filter: PendingTxFilter,
) -> Result<UnboundedReceiver<PendingTxEvent>, DomainError> {
    port.subscribe(chain, filter).await
}

/// Forward a new filter predicate to `port`. The composition root
/// drains the MempoolScreen's control channel and calls this helper;
/// failures surface as [`DomainError`] and should be logged but not
/// bubbled up to the UI (client-side prune already happened).
pub async fn update_filter<P: PendingTxStreamPort>(
    port: &P,
    chain: Chain,
    filter: PendingTxFilter,
) -> Result<(), DomainError> {
    port.update_filter(chain, filter).await
}
