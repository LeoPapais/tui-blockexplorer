//! Background task for the Address Detail screen.
//!
//! Spawns a single Tokio task that, for each incoming `Address`,
//! fans out the overview fetch and the transfers fetch in parallel
//! (`tokio::join!`) and forwards each result on its own channel so
//! the UI can render them as soon as they arrive.
//!
//! See `plan/6-address-detail.md` sections 12.3 and 12.4.1.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::AddressFeedSender,
    application::ports::{AddressReaderPort, TransfersPort},
    domain::Chain,
};

/// Spawn the address-detail feed with both an address reader and a
/// transfers provider. The transfers stream drives the Transactions
/// tab.
pub fn spawn<R, T>(
    chain: Chain,
    reader: R,
    transfers: T,
    sender: AddressFeedSender,
) -> JoinHandle<()>
where
    R: AddressReaderPort + Clone + 'static,
    T: TransfersPort + Clone + 'static,
{
    tokio::spawn(async move {
        let AddressFeedSender {
            updates_tx,
            transfers_tx,
            mut input_rx,
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            let reader = reader.clone();
            let transfers = transfers.clone();
            let (ov_res, tr_res) = tokio::join!(
                reader.get(addr, chain),
                transfers.get_for_address(addr, chain, None),
            );
            if let Ok(Some(ov)) = ov_res
                && updates_tx.send(ov).is_err()
            {
                break;
            }
            if let Ok(page) = tr_res
                && transfers_tx.send(page).is_err()
            {
                break;
            }
        }
    })
}
