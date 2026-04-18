//! Background task for the Address Detail screen.
//!
//! Spawns a single Tokio task that, for each incoming `Address`,
//! fans out the overview / transfers / portfolio fetches in parallel
//! and forwards each result on its own channel so the UI can render
//! them progressively.
//!
//! When the address turns out to be a contract, the task fires a
//! follow-up ERC-20 probe (`TokenReaderPort::get`) and, if positive,
//! a price + history fetch for the inline Token tab. The probe is
//! **pessimistic**: it runs only when `AddressOverview.kind == Contract`,
//! so EOAs never pay any extra call.
//!
//! See `plan/6-address-detail.md` sections 12.3, 12.4.1, 12.4.2 and
//! 12.4.4.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::AddressFeedSender,
    application::{
        ports::{
            AddressReaderPort, EnsResolverPort, PortfolioPort, PricesPort, TokenReaderPort,
            TransfersPort,
        },
        use_cases::{load_address_overview, load_address_portfolio},
    },
    domain::{AddressKind, Chain, PriceWindow},
};

/// Spawn the address-detail feed with every port it needs. The
/// `token_reader` and `prices` arguments are always passed in even
/// for EOAs: their calls are gated inside the task so the trait
/// bounds stay uniform across the BDD and production wiring.
///
/// `ens` backs reverse-ENS resolution: the task calls
/// `load_address_overview::run` which overlays the friendly name
/// on top of the reader's result. See `plan/6-address-detail.md`
/// §11 "Shipped".
#[allow(clippy::too_many_arguments)]
pub fn spawn<R, T, P, K, Pr, E>(
    chain: Chain,
    reader: R,
    transfers: T,
    portfolio: P,
    token_reader: K,
    prices: Pr,
    ens: E,
    sender: AddressFeedSender,
) -> JoinHandle<()>
where
    R: AddressReaderPort + Clone + 'static,
    T: TransfersPort + Clone + 'static,
    P: PortfolioPort + Clone + 'static,
    K: TokenReaderPort + Clone + 'static,
    Pr: PricesPort + Clone + 'static,
    E: EnsResolverPort + Clone + 'static,
{
    tokio::spawn(async move {
        let AddressFeedSender {
            updates_tx,
            transfers_tx,
            portfolio_tx,
            token_overview_tx,
            token_price_tx,
            token_series_tx,
            mut input_rx,
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            let reader = reader.clone();
            let transfers = transfers.clone();
            let portfolio = portfolio.clone();
            let prices_for_portfolio = prices.clone();
            let ens = ens.clone();
            // `load_address_portfolio::run` fans out one spot-price
            // lookup per holding so the Tokens tab can render real
            // USD values (plan/15-backlog.md §3.4 + plan/6 §11
            // "Shipped" — Portfolio USD totals).
            let (ov_res, tr_res, pf_res) = tokio::join!(
                load_address_overview::run(&reader, &ens, addr, chain),
                transfers.get_for_address(addr, chain, None),
                load_address_portfolio::run(&portfolio, &prices_for_portfolio, addr, chain),
            );

            // Forward the three "always" results first.
            let overview_clone = match ov_res.as_ref() {
                Ok(ov) => Some(ov.clone()),
                Err(_) => None,
            };
            if let Some(ov) = overview_clone.clone()
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

            // Gate: only probe ERC-20 metadata when we are sure the
            // address is a contract. EOAs fall out here with zero
            // extra RPC calls.
            let Some(overview) = overview_clone else {
                continue;
            };
            if !matches!(overview.kind, AddressKind::Contract) {
                continue;
            }

            match token_reader.get(addr, chain).await {
                Ok(Some(token_overview)) => {
                    if token_overview_tx.send(Some(token_overview)).is_err() {
                        break;
                    }
                    // Fire spot price + D1 history in parallel once
                    // we know the address is an ERC-20.
                    let prices_single = prices.clone();
                    let prices_hist = prices.clone();
                    let (price_res, series_res) = tokio::join!(
                        prices_single.get_single(addr, chain),
                        prices_hist.get_history(addr, chain, PriceWindow::D1),
                    );
                    // plan/15-backlog.md §3.4: the port hands back a
                    // full PriceLookup (Available / Unsupported);
                    // forward it unchanged so the UI flips out of
                    // the pending state.
                    if let Ok(lookup) = price_res
                        && token_price_tx.send(lookup).is_err()
                    {
                        break;
                    }
                    if let Ok(series) = series_res
                        && token_series_tx.send(series).is_err()
                    {
                        break;
                    }
                }
                Ok(None) => {
                    let _ = token_overview_tx.send(None);
                }
                Err(_) => {
                    let _ = token_overview_tx.send(None);
                }
            }
        }
    })
}
