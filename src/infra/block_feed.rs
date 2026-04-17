//! Background task that answers `BlockId` navigation requests with
//! fully-fetched `Block` values. Mirrors `search_feed.rs` for the
//! Block Detail screen.
//!
//! See `plan/3-block-detail.md` section 11.3.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::BlockFeedSender, application::ports::BlockReaderPort, domain::Chain,
};

pub fn spawn<R>(chain: Chain, reader: R, sender: BlockFeedSender) -> JoinHandle<()>
where
    R: BlockReaderPort + 'static,
{
    tokio::spawn(async move {
        let BlockFeedSender {
            updates_tx,
            mut input_rx,
        } = sender;
        // Silently ignore missing blocks and provider errors: the
        // screen keeps rendering the last block it successfully
        // received.
        while let Some(id) = input_rx.recv().await {
            if let Ok(Some(block)) = reader.get(id, chain).await
                && updates_tx.send(block).is_err()
            {
                break;
            }
        }
    })
}
