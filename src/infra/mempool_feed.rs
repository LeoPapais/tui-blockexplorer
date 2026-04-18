//! Adapters backing the Mempool screen for the `cargo run` path.
//!
//! Exposes:
//!
//! * [`EmptyPendingTxStream`] — the no-op stream used until the
//!   live WS adapter is wired end-to-end. Subscribers get a
//!   receiver whose sender is dropped immediately, so the Mempool
//!   screen renders its "waiting..." state indefinitely.
//! * [`spawn_filter_drain`] — bridge between the Mempool screen's
//!   filter control channel and `PendingTxStreamPort::update_filter`
//!   (see `plan/5-mempool.md` §11.3.2).
//! * [`mempool_status_feed`] — channel used to surface
//!   connection-state transitions to the screen's reconnecting
//!   badge (see `plan/5-mempool.md` §11.3.3).

use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
    task::JoinHandle,
};

use crate::{
    application::{
        ConnectionStatus,
        ports::PendingTxStreamPort,
        use_cases::observe_pending_txs,
    },
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

    async fn update_filter(
        &self,
        _chain: Chain,
        _filter: PendingTxFilter,
    ) -> Result<(), DomainError> {
        // The empty stream never receives events so filter updates
        // are meaningless; treat the call as a successful no-op.
        Ok(())
    }
}

/// Spawn a task that drains filter updates from the Mempool screen's
/// control channel and forwards them to
/// `PendingTxStreamPort::update_filter` on the given chain. Returns
/// the sender half that `MempoolScreen::with_filter_control` expects
/// and a join handle the caller can keep to observe task lifetime.
///
/// The task terminates quietly when the sender is dropped (the
/// Mempool screen was popped) or when `port.update_filter` returns
/// [`DomainError::ProviderUnavailable`] repeatedly — today it just
/// ignores the error so a single flaky upstream does not kill the
/// whole bridge.
///
/// See `plan/5-mempool.md` §11.3.2.
pub fn spawn_filter_drain<P>(
    port: P,
    chain: Chain,
) -> (UnboundedSender<PendingTxFilter>, JoinHandle<()>)
where
    P: PendingTxStreamPort + 'static,
{
    let (tx, mut rx) = unbounded_channel::<PendingTxFilter>();
    let handle = tokio::spawn(async move {
        while let Some(filter) = rx.recv().await {
            let _ = observe_pending_txs::update_filter(&port, chain, filter).await;
        }
    });
    (tx, handle)
}

/// Receiving end of the status feed used by the reconnecting badge.
/// Constructed via [`mempool_status_feed`]. The composition root
/// publishes `ConnectionStatus` updates on the sender; the screen
/// drains the receiver on every tick.
///
/// See `plan/5-mempool.md` §11.3.3.
pub type MempoolStatusFeed = UnboundedReceiver<ConnectionStatus>;

/// Sending end of the status feed. Cloneable because multiple
/// supervisors (subscribe + update_filter) might want to publish the
/// same status concurrently.
pub type MempoolStatusSender = UnboundedSender<ConnectionStatus>;

/// Build a new `(MempoolStatusFeed, MempoolStatusSender)` pair. The
/// screen takes the receiver via `MempoolScreen::with_status_feed`;
/// whatever component owns the WebSocket lifecycle publishes updates
/// on the sender.
#[must_use]
pub fn mempool_status_feed() -> (MempoolStatusFeed, MempoolStatusSender) {
    let (tx, rx) = unbounded_channel();
    (rx, tx)
}
