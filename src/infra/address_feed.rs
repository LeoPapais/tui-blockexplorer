//! Background task for the unified Address Detail screen.
//!
//! Spawns a single Tokio task that composes the feeds that used to
//! live in `contract_feed.rs` and `token_feed.rs`. Lazy dispatch
//! keeps the CU budget flat for EOAs: the contract-gated and
//! ERC-20-gated branches only run after the first overview confirms
//! the address is a contract (resp. an ERC-20 contract).
//!
//! See `plan/16-unified-address-detail.md` §5 (feed composition)
//! and §6 (sequence diagram).

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::{
        AddressFeedSender,
        address_detail::ReadDelivery,
    },
    application::{
        ports::{
            AddressReaderPort, ContractReaderPort, ContractSourcePort, EnsResolverPort,
            EventLogPort, NetworkStatusPort, PortfolioPort, PricesPort, ProxyDetectionPort,
            StoragePort, TokenPriceStreamPort, TokenReaderPort, TransfersPort,
        },
        use_cases::{
            load_address_overview, load_address_portfolio, load_contract_events_page,
            load_contract_overview,
        },
    },
    domain::{Address, AddressKind, Chain, PriceWindow},
};

/// Wire every port needed by `AddressDetailScreen`. The task
/// branches internally on `AddressKind` / ERC-20 probe to keep EOAs
/// at zero extra RPC calls.
#[allow(clippy::too_many_arguments)]
pub fn spawn<R, T, P, K, Pr, E, Pd, S, Cr, El, St, N, Ps>(
    chain: Chain,
    reader: R,
    transfers: T,
    portfolio: P,
    token_reader: K,
    prices: Pr,
    ens: E,
    proxy_detector: Pd,
    source: S,
    contract_reader: Cr,
    event_log: El,
    storage: St,
    network_status: N,
    price_stream: Ps,
    sender: AddressFeedSender,
) -> JoinHandle<()>
where
    R: AddressReaderPort + Clone + Send + Sync + 'static,
    T: TransfersPort + Clone + Send + Sync + 'static,
    P: PortfolioPort + Clone + Send + Sync + 'static,
    K: TokenReaderPort + Clone + Send + Sync + 'static,
    Pr: PricesPort + Clone + Send + Sync + 'static,
    E: EnsResolverPort + Clone + Send + Sync + 'static,
    Pd: ProxyDetectionPort + Clone + Send + Sync + 'static,
    S: ContractSourcePort + Clone + Send + Sync + 'static,
    Cr: ContractReaderPort + Clone + Send + Sync + 'static,
    El: EventLogPort + Clone + Send + Sync + 'static,
    St: StoragePort + Clone + Send + Sync + 'static,
    N: NetworkStatusPort + Clone + Send + Sync + 'static,
    Ps: TokenPriceStreamPort + Clone + Send + Sync + 'static,
{
    tokio::spawn(async move {
        let AddressFeedSender {
            updates_tx,
            transfers_tx,
            portfolio_tx,
            token_overview_tx,
            token_price_tx,
            token_series_tx,
            mut token_window_req_rx,
            token_transfers_tx,
            contract_overview_tx,
            source_tx,
            contract_impl_overview_tx,
            source_impl_tx,
            mut read_rx,
            read_tx,
            mut events_rx,
            events_tx,
            mut storage_rx,
            storage_tx,
            mut input_rx,
        } = sender;

        let mut active: Option<Address> = None;
        let mut active_kind_is_contract = false;
        let mut active_is_token = false;
        let mut price_stream_rx: Option<
            tokio::sync::mpsc::UnboundedReceiver<crate::domain::PriceLookup>,
        > = None;

        loop {
            tokio::select! {
                // Bias towards new addresses so `active` is always
                // populated before a window / read / events / storage
                // request is processed.
                biased;
                maybe_addr = input_rx.recv() => {
                    let Some(addr) = maybe_addr else { break; };
                    active = Some(addr);
                    active_kind_is_contract = false;
                    active_is_token = false;
                    price_stream_rx = None;

                    // Always-on fan-out.
                    let reader_cl = reader.clone();
                    let transfers_cl = transfers.clone();
                    let portfolio_cl = portfolio.clone();
                    let prices_for_portfolio = prices.clone();
                    let ens_cl = ens.clone();
                    let (ov_res, tr_res, pf_res) = tokio::join!(
                        load_address_overview::run(&reader_cl, &ens_cl, addr, chain),
                        transfers_cl.get_for_address(addr, chain, None),
                        load_address_portfolio::run(&portfolio_cl, &prices_for_portfolio, addr, chain),
                    );

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

                    let Some(overview) = overview_clone else { continue };
                    if !matches!(overview.kind, AddressKind::Contract) {
                        continue;
                    }
                    active_kind_is_contract = true;

                    // Contract-gated branch: overview + source.
                    if let Ok(cov) = load_contract_overview::run(
                        &reader, &proxy_detector, addr, chain,
                    )
                    .await
                    {
                        if contract_overview_tx.send(cov.clone()).is_err() {
                            break;
                        }
                        if let Some(px) = cov.proxy {
                            let impl_addr = px.implementation;
                            if let Ok(impl_cov) = load_contract_overview::run(
                                &reader,
                                &proxy_detector,
                                impl_addr,
                                chain,
                            )
                            .await
                                && contract_impl_overview_tx.send(impl_cov).is_err()
                            {
                                break;
                            }
                            if let Ok(Some(src_impl)) =
                                source.get_source(impl_addr, chain).await
                                && source_impl_tx.send(src_impl).is_err()
                            {
                                break;
                            }
                        }
                    }
                    if let Ok(Some(src)) = source.get_source(addr, chain).await
                        && source_tx.send(src).is_err()
                    {
                        break;
                    }

                    // ERC-20 probe.
                    match token_reader.get(addr, chain).await {
                        Ok(Some(token_overview)) => {
                            active_is_token = true;
                            if token_overview_tx.send(Some(token_overview)).is_err() {
                                break;
                            }
                            let prices_single = prices.clone();
                            let prices_hist = prices.clone();
                            let transfers_for_token = transfers.clone();
                            let (price_res, series_res, token_tr_res) = tokio::join!(
                                prices_single.get_single(addr, chain),
                                prices_hist.get_history(addr, chain, PriceWindow::D1),
                                transfers_for_token.get_for_contract(addr, chain, None),
                            );
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
                            if let Ok(page) = token_tr_res
                                && token_transfers_tx.send(page).is_err()
                            {
                                break;
                            }
                            // Subscribe to the live price stream
                            // once the static fetches are done, so
                            // the warm-up sample does not race the
                            // one-shot price.
                            price_stream_rx = price_stream.subscribe(addr, chain).await.ok();
                        }
                        _ => {
                            let _ = token_overview_tx.send(None);
                        }
                    }
                }
                maybe_window = token_window_req_rx.recv() => {
                    let Some(window) = maybe_window else { break; };
                    let Some(addr) = active else { continue; };
                    if !active_is_token { continue; }
                    let series = prices
                        .get_history(addr, chain, window)
                        .await
                        .unwrap_or_else(|_| crate::domain::PriceSeries::empty(window));
                    if token_series_tx.send(series).is_err() {
                        break;
                    }
                }
                req = read_rx.recv() => {
                    let Some(req) = req else { break };
                    let Some(addr) = active else { continue };
                    if !active_kind_is_contract { continue; }
                    let crate::adapters::ui::address_detail::ReadRequest {
                        function,
                        args,
                        calldata_source,
                    } = req;
                    let signature = function.signature();
                    // `addr` is the proxy / user-facing contract; impl-tab Read uses
                    // [`ReadCalldataSource::ImplementationArtifact`] only to route UI
                    // state — `ContractReaderPort::call` still receives `addr`.
                    let result = contract_reader
                        .call(addr, chain, &function, args)
                        .await;
                    if read_tx
                        .send(ReadDelivery {
                            calldata_source,
                            signature,
                            result,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                req = events_rx.recv() => {
                    let Some(req) = req else { break };
                    let Some(addr) = active else { continue };
                    if !active_kind_is_contract { continue; }
                    let result = load_contract_events_page::run(
                        &network_status,
                        &event_log,
                        addr,
                        chain,
                        req.head_hint,
                        req.offset,
                    )
                    .await;
                    if events_tx.send(result).is_err() {
                        break;
                    }
                }
                req = storage_rx.recv() => {
                    let Some(req) = req else { break };
                    let Some(addr) = active else { continue };
                    if !active_kind_is_contract { continue; }
                    let result = storage.get_at(addr, chain, req.slot).await;
                    if storage_tx.send(result).is_err() {
                        break;
                    }
                }
                maybe_lookup = async {
                    match price_stream_rx.as_mut() {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match maybe_lookup {
                        Some(lookup) => {
                            if token_price_tx.send(lookup).is_err() {
                                break;
                            }
                        }
                        None => {
                            price_stream_rx = None;
                        }
                    }
                }
            }
        }
    })
}
