//! Background task that answers Address requests with a
//! `TokenOverview`. Mirrors the other `*_feed.rs` helpers.
//!
//! See `plan/8-token-detail.md` section 12.3.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::TokenFeedSender, application::ports::TokenReaderPort, domain::Chain,
};

pub fn spawn<R>(chain: Chain, reader: R, sender: TokenFeedSender) -> JoinHandle<()>
where
    R: TokenReaderPort + 'static,
{
    tokio::spawn(async move {
        let TokenFeedSender {
            updates_tx,
            mut input_rx,
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            if let Ok(Some(ov)) = reader.get(addr, chain).await
                && updates_tx.send(ov).is_err()
            {
                break;
            }
        }
    })
}
