//! Background task that answers `TxHash` requests with full
//! `Transaction` values. Mirrors `block_feed.rs` and `search_feed.rs`
//! for the TxDetail screen.
//!
//! See `plan/4-tx-detail.md` section 12.3.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::TxFeedSender, application::ports::TxReaderPort, domain::Chain,
};

pub fn spawn<R>(chain: Chain, reader: R, sender: TxFeedSender) -> JoinHandle<()>
where
    R: TxReaderPort + 'static,
{
    tokio::spawn(async move {
        let TxFeedSender {
            updates_tx,
            mut input_rx,
        } = sender;
        // Silently ignore missing txs and provider errors.
        while let Some(hash) = input_rx.recv().await {
            if let Ok(Some(tx)) = reader.get(hash, chain).await
                && updates_tx.send(tx).is_err()
            {
                break;
            }
        }
    })
}
