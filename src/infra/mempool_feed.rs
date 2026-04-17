//! Adapters backing the Mempool screen for the `cargo run` path.
//!
//! For now this module only exposes `EmptyPendingTxStream`, a port
//! implementation that produces no events. When the Alchemy
//! WebSocket adapter lands (see `plan/5-mempool.md` section 11.3) it
//! replaces this file without touching `infra::run`.

use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use crate::{
    application::ports::PendingTxStreamPort,
    domain::{Chain, DomainError, PendingTxEvent, PendingTxFilter},
};

/// No-op stream used until the WS adapter lands. Subscribers get a
/// receiver whose sender is dropped immediately, so the Mempool
/// screen renders its "waiting..." state indefinitely.
#[derive(Debug, Default, Clone, Copy)]
pub struct EmptyPendingTxStream;

impl PendingTxStreamPort for EmptyPendingTxStream {
    async fn subscribe(
        &self,
        _chain: Chain,
        _filter: PendingTxFilter,
    ) -> Result<UnboundedReceiver<PendingTxEvent>, DomainError> {
        let (_, rx) = unbounded_channel();
        Ok(rx)
    }
}
