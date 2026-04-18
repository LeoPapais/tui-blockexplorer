//! Background task that feeds the Token Detail screen.
//!
//! On every address received on `input_rx` the task fans out four
//! parallel calls:
//!
//! - `TokenReaderPort::get`       → overview (metadata + supply).
//! - `PricesPort::get_single`     → spot-price status
//!   (`PriceLookup::Available` / `Unsupported` — see
//!   `plan/15-backlog.md` §3.4).
//! - `PricesPort::get_history(D1)` → default chart window series.
//! - `TransfersPort::get_for_contract` → recent ERC-20 transfers.
//!
//! It also listens on `window_req_rx` for additional `PriceWindow`
//! requests (e.g. the user switching to M1 / Y1) and answers each
//! one with another `PricesPort::get_history` call. The window
//! request loop is independent of the address loop so the chart
//! stays responsive.
//!
//! The `select!` is **biased** and checks `input_rx` first: this
//! guarantees that when an address and a window request race (which
//! happens on screen construction, when the screen publishes both
//! the address and a pending window request in quick succession)
//! the address is processed first and `current_address` is set
//! before any window request ever runs. Combined with the automatic
//! D1 fetch inside `run_for_address`, this removes the "Loading 1d
//! price history..." deadlock that could previously happen when
//! `select!` picked the window branch first and dropped the
//! request because no address was known yet.
//!
//! See `plan/8-token-detail.md` section 12.4.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::TokenFeedSender,
    application::ports::{PricesPort, TokenReaderPort, TransfersPort},
    domain::{Address, Chain, PriceWindow},
};

/// Wire every port needed by the TokenDetailScreen. The D1 chart
/// window is fetched automatically on every incoming address;
/// callers only need to push additional `PriceWindow` values when
/// the user switches to M1 or Y1.
pub fn spawn<R, P, T>(
    chain: Chain,
    reader: R,
    prices: P,
    transfers: T,
    sender: TokenFeedSender,
) -> JoinHandle<()>
where
    R: TokenReaderPort + Clone + 'static,
    P: PricesPort + Clone + 'static,
    T: TransfersPort + Clone + 'static,
{
    tokio::spawn(async move {
        let TokenFeedSender {
            updates_tx,
            price_tx,
            transfers_tx,
            history_tx,
            mut input_rx,
            mut window_req_rx,
        } = sender;

        let mut current_address: Option<Address> = None;

        loop {
            tokio::select! {
                // Bias: new addresses always win over pending window
                // requests so `current_address` is populated before
                // any `window_req` branch ever runs.
                biased;
                maybe_addr = input_rx.recv() => {
                    let Some(addr) = maybe_addr else { break; };
                    current_address = Some(addr);
                    run_for_address(
                        addr,
                        chain,
                        reader.clone(),
                        prices.clone(),
                        transfers.clone(),
                        &updates_tx,
                        &price_tx,
                        &transfers_tx,
                        &history_tx,
                    )
                    .await;
                }
                maybe_window = window_req_rx.recv() => {
                    let Some(window) = maybe_window else { break; };
                    let Some(addr) = current_address else { continue; };
                    let series = prices
                        .get_history(addr, chain, window)
                        .await
                        .unwrap_or_else(|_| {
                            crate::domain::PriceSeries::empty(window)
                        });
                    if history_tx.send(series).is_err() {
                        break;
                    }
                }
            }
        }
    })
}

#[allow(clippy::too_many_arguments)]
async fn run_for_address<R, P, T>(
    address: Address,
    chain: Chain,
    reader: R,
    prices: P,
    transfers: T,
    updates_tx: &tokio::sync::mpsc::UnboundedSender<crate::domain::TokenOverview>,
    price_tx: &tokio::sync::mpsc::UnboundedSender<crate::domain::PriceLookup>,
    transfers_tx: &tokio::sync::mpsc::UnboundedSender<crate::domain::TransferPage>,
    history_tx: &tokio::sync::mpsc::UnboundedSender<crate::domain::PriceSeries>,
) where
    R: TokenReaderPort + 'static,
    P: PricesPort + Clone + 'static,
    T: TransfersPort + 'static,
{
    // The D1 history is always refreshed alongside the other three
    // fetches so the Chart tab never gets stuck on the "loading..."
    // state regardless of tokio::select! ordering.
    let prices_hist = prices.clone();
    let (ov_res, price_res, tr_res, hist_res) = tokio::join!(
        reader.get(address, chain),
        prices.get_single(address, chain),
        transfers.get_for_contract(address, chain, None),
        prices_hist.get_history(address, chain, PriceWindow::D1),
    );

    if let Ok(Some(ov)) = ov_res {
        let _ = updates_tx.send(ov);
    }
    // Forward whatever the port returned: Available / Unsupported
    // both flip the UI out of the "loading" state. Transport
    // failures keep the price pending (the user can retry).
    if let Ok(lookup) = price_res {
        let _ = price_tx.send(lookup);
    }
    if let Ok(page) = tr_res {
        let _ = transfers_tx.send(page);
    }
    // Always publish a D1 series, even if it is empty — the UI
    // handles the empty case with a dedicated "no price data"
    // message instead of staying stuck in loading.
    let series = hist_res.unwrap_or_else(|_| crate::domain::PriceSeries::empty(PriceWindow::D1));
    let _ = history_tx.send(series);
}
