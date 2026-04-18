//! Background task for the Address Detail screen.
//!
//! Spawns a single Tokio task that, for each incoming `Address`,
//! fans out the overview, transfers and portfolio fetches in parallel
//! (`tokio::join!`) and forwards each result on its own channel so
//! the UI can render them as soon as they arrive.
//!
//! See `plan/6-address-detail.md` sections 12.3, 12.4.1 and 12.4.2.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::AddressFeedSender,
    application::ports::{AddressReaderPort, PortfolioPort, TransfersPort},
    domain::Chain,
};

/// Spawn the address-detail feed with an address reader, a transfers
/// provider, and a portfolio provider. Each result streams into the
/// UI on its own channel so tabs render progressively.
pub fn spawn<R, T, P>(
    chain: Chain,
    reader: R,
    transfers: T,
    portfolio: P,
    sender: AddressFeedSender,
) -> JoinHandle<()>
where
    R: AddressReaderPort + Clone + 'static,
    T: TransfersPort + Clone + 'static,
    P: PortfolioPort + Clone + 'static,
{
    tokio::spawn(async move {
        let AddressFeedSender {
            updates_tx,
            transfers_tx,
            portfolio_tx,
            mut input_rx,
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            let reader = reader.clone();
            let transfers = transfers.clone();
            let portfolio = portfolio.clone();
            let (ov_res, tr_res, pf_res) = tokio::join!(
                reader.get(addr, chain),
                transfers.get_for_address(addr, chain, None),
                portfolio.get_token_balances(addr, chain),
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
            if let Ok(holdings) = pf_res
                && portfolio_tx.send(holdings).is_err()
            {
                break;
            }
        }
    })
}
