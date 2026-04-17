//! Use case that opens a pending-tx subscription through the port and
//! returns the resulting receiver to the caller.
//!
//! See `plan/5-mempool.md` section 11.1.

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
