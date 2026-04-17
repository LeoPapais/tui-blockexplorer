//! Background task that answers `Address` requests with full
//! `AddressOverview` values. Mirrors `block_feed.rs` / `tx_feed.rs`
//! for the Address Detail screen.
//!
//! See `plan/6-address-detail.md` section 12.3.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::AddressFeedSender, application::ports::AddressReaderPort, domain::Chain,
};

pub fn spawn<R>(chain: Chain, reader: R, sender: AddressFeedSender) -> JoinHandle<()>
where
    R: AddressReaderPort + 'static,
{
    tokio::spawn(async move {
        let AddressFeedSender {
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
