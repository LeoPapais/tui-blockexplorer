//! Step definitions for the Search feature.
//!
//! Each scenario drives the real SearchScreen/HomeScreen through the
//! stub ports held by the cucumber world. The scenarios live in
//! `tests/e2e/features/search.feature`; the underlying logic lives in
//! `plan/2-search.md` section 10.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{
        AddressDetailScreen, AddressTab, BlockDetailScreen, Command, DetailPlaceholderScreen,
        HomeScreen, ScreenStack, SearchScreen, TxDetailScreen, address_feed, block_feed, home_feed,
        search_feed, tx_feed,
    },
    application::{
        ConnectionStatus, HomeViewModel,
        ports::{
            AddressReaderPort, BlockReaderPort, ProxyDetectionPort, TokenReaderPort, TxReaderPort,
        },
        use_cases::load_contract_overview,
    },
    domain::{
        Address, AddressKind, BlockHash, BlockId, BlockNumber, BlockSummary, Chain, ResolvedEntity,
        TxHash, TxSummary,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use cucumber::{given, then, when};
use pretty_assertions::assert_eq;

use crate::world::AppWorld;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ensure_active_chain(world: &mut AppWorld) -> Chain {
    *world.active_chain.get_or_insert(Chain::Ethereum)
}

fn initial_home_view(chain: Chain) -> HomeViewModel {
    HomeViewModel {
        chain,
        network: None,
        gas: None,
        connection: ConnectionStatus::Connected,
    }
}

pub(crate) fn build_stack(world: &mut AppWorld) {
    if world.stack.is_some() {
        return;
    }
    let chain = ensure_active_chain(world);

    let (home_feed_rx, _home_feed_tx) = home_feed();
    let search_factory = build_search_factory(world, chain);

    let home = HomeScreen::with_feed(initial_home_view(chain), home_feed_rx)
        .with_search_factory(search_factory);

    let mut stack = ScreenStack::new();
    stack.push(Box::new(home));
    world.stack = Some(stack);
}

pub(crate) fn build_search_factory(
    world: &AppWorld,
    chain: Chain,
) -> Box<dyn Fn() -> Box<dyn blockexplorer_tui::adapters::ui::Screen> + Send + 'static> {
    let block = world.block_stub.clone();
    let tx = world.tx_stub.clone();
    let address = world.address_stub.clone();
    let ens = world.ens_stub.clone();
    let token = world.token_stub.clone();
    let block_reader = world.block_reader_stub.clone();
    let tx_reader = world.tx_reader_stub.clone();
    let address_reader = world.address_reader_stub.clone();
    let proxy_detector = world.proxy_detector_stub.clone();
    let token_reader = world.token_reader_stub.clone();
    let search_cache = world.search_cache.clone();

    Box::new(move || {
        let (feed, sender) = search_feed();
        let block = block.clone();
        let tx = tx.clone();
        let address = address.clone();
        let ens = ens.clone();
        let token = token.clone();
        let token_reader_probe = token_reader.clone();
        let block_reader_for_detail = block_reader.clone();
        let tx_reader_for_detail = tx_reader.clone();
        let address_reader_for_detail = address_reader.clone();
        let proxy_detector_for_detail = proxy_detector.clone();
        let token_reader_for_detail = token_reader.clone();
        let cache_for_spawn = search_cache.clone();

        // Reuse the production search feed so the BDD scenarios
        // cover the same two-phase emission path used live.
        std::mem::drop(blockexplorer_tui::infra::search_feed::spawn_with_cache(
            chain,
            block,
            tx,
            address,
            ens,
            token,
            token_reader_probe,
            sender,
            cache_for_spawn,
        ));

        let detail_factory: Box<
            dyn Fn(ResolvedEntity) -> Box<dyn blockexplorer_tui::adapters::ui::Screen>
                + Send
                + 'static,
        > = {
            let block_reader = block_reader_for_detail.clone();
            let tx_reader = tx_reader_for_detail.clone();
            let address_reader = address_reader_for_detail.clone();
            let proxy_detector = proxy_detector_for_detail.clone();
            let token_reader = token_reader_for_detail.clone();
            Box::new(move |entity: ResolvedEntity| match entity {
                ResolvedEntity::Block { number, .. } => spawn_block_detail(
                    chain,
                    BlockId::Number(number),
                    block_reader.clone(),
                    tx_reader.clone(),
                ),
                ResolvedEntity::Tx { hash, .. } => spawn_tx_detail(chain, hash, tx_reader.clone()),
                ResolvedEntity::Address { address, kind, .. } => match kind {
                    AddressKind::Contract => spawn_contract_detail(
                        chain,
                        address,
                        address_reader.clone(),
                        proxy_detector.clone(),
                    ),
                    AddressKind::Eoa { .. } => {
                        spawn_address_detail(chain, address, address_reader.clone())
                    }
                },
                ResolvedEntity::Contract { address } => spawn_contract_detail(
                    chain,
                    address,
                    address_reader.clone(),
                    proxy_detector.clone(),
                ),
                ResolvedEntity::DelegatedEoa { address, .. } => {
                    spawn_address_detail(chain, address, address_reader.clone())
                }
                ResolvedEntity::Token(meta) => spawn_token_detail(
                    chain,
                    meta.address,
                    token_reader.clone(),
                    address_reader.clone(),
                ),
                other => Box::new(DetailPlaceholderScreen::new(other))
                    as Box<dyn blockexplorer_tui::adapters::ui::Screen>,
            })
        };

        Box::new(SearchScreen::new(feed, detail_factory))
    })
}

pub(crate) fn spawn_block_detail<
    B: BlockReaderPort + Clone + 'static,
    T: TxReaderPort + Clone + 'static,
>(
    chain: Chain,
    id: BlockId,
    block_reader: B,
    tx_reader: T,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    let (feed, sender) = block_feed();
    let reader_for_task = block_reader.clone();
    tokio::spawn(async move {
        let blockexplorer_tui::adapters::ui::BlockFeedSender {
            updates_tx,
            mut input_rx,
        } = sender;
        while let Some(bid) = input_rx.recv().await {
            if let Ok(Some(block)) = reader_for_task.get(bid, chain).await
                && updates_tx.send(block).is_err()
            {
                break;
            }
        }
    });
    let tx_reader_for_open = tx_reader.clone();
    let open_tx = Box::new(move |hash| spawn_tx_detail(chain, hash, tx_reader_for_open.clone()));
    Box::new(BlockDetailScreen::loading(chain, id, feed, open_tx))
}

/// Spawn a unified AddressDetail focused on the Contract tab.
/// Feeds the contract-overview channel (Overview sub-tab) by
/// loading the ContractOverview through the use case. Other
/// contract-specific channels (source, read, events, storage) stay
/// unanswered, matching the old minimal `spawn_contract_detail`
/// helper's shape.
pub(crate) fn spawn_contract_detail<
    A: AddressReaderPort + Clone + 'static,
    P: ProxyDetectionPort + Clone + 'static,
>(
    chain: Chain,
    address: Address,
    address_reader: A,
    proxy_detector: P,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    let (feed, sender) = address_feed();
    let reader = address_reader.clone();
    let detector = proxy_detector.clone();
    tokio::spawn(async move {
        let blockexplorer_tui::adapters::ui::AddressFeedSender {
            updates_tx,
            contract_overview_tx,
            mut input_rx,
            ..
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            // First publish the AddressOverview so the screen can
            // reveal the Contract tab.
            if let Ok(Some(ov)) = reader.get(addr, chain).await
                && updates_tx.send(ov).is_err()
            {
                break;
            }
            if let Ok(cov) = load_contract_overview::run(&reader, &detector, addr, chain).await
                && contract_overview_tx.send(cov).is_err()
            {
                break;
            }
        }
    });
    Box::new(AddressDetailScreen::with_factories_and_tab(
        chain,
        address,
        feed,
        None,
        None,
        AddressTab::Contract,
    ))
}

/// Fully-wired unified AddressDetail focused on the Contract tab.
/// Wires every contract-specific port so the full Source / ABI /
/// Read / Events / Storage sub-tab surface is driveable.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn spawn_address_detail_as_contract_full<
    A: AddressReaderPort + Clone + Send + Sync + 'static,
    Pr: ProxyDetectionPort + Clone + Send + Sync + 'static,
    S: blockexplorer_tui::application::ports::ContractSourcePort + Clone + Send + Sync + 'static,
    CR: blockexplorer_tui::application::ports::ContractReaderPort + Clone + Send + Sync + 'static,
    E: blockexplorer_tui::application::ports::EventLogPort + Clone + Send + Sync + 'static,
    St: blockexplorer_tui::application::ports::StoragePort + Clone + Send + Sync + 'static,
    N: blockexplorer_tui::application::ports::NetworkStatusPort + Clone + Send + Sync + 'static,
>(
    chain: Chain,
    address: Address,
    address_reader: A,
    proxy_detector: Pr,
    source: S,
    contract_reader: CR,
    event_log: E,
    storage: St,
    network_status: N,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    use blockexplorer_tui::application::use_cases::load_contract_events_page;
    let (feed, sender) = address_feed();
    tokio::spawn(async move {
        let blockexplorer_tui::adapters::ui::AddressFeedSender {
            updates_tx,
            contract_overview_tx,
            source_tx,
            mut read_rx,
            read_tx,
            mut events_rx,
            events_tx,
            mut storage_rx,
            storage_tx,
            mut input_rx,
            ..
        } = sender;
        let mut active: Option<Address> = None;
        loop {
            tokio::select! {
                maybe_addr = input_rx.recv() => {
                    let Some(addr) = maybe_addr else { break; };
                    active = Some(addr);
                    if let Ok(Some(ov)) = address_reader.get(addr, chain).await
                        && updates_tx.send(ov).is_err()
                    {
                        break;
                    }
                    if let Ok(cov) = load_contract_overview::run(
                        &address_reader, &proxy_detector, addr, chain,
                    ).await
                        && contract_overview_tx.send(cov).is_err()
                    {
                        break;
                    }
                    if let Ok(Some(src)) = source.get_source(addr, chain).await
                        && source_tx.send(src).is_err()
                    {
                        break;
                    }
                }
                req = read_rx.recv() => {
                    let Some(req) = req else { break; };
                    let Some(a) = active else { continue; };
                    let result = contract_reader.call(a, chain, &req.function, req.args).await;
                    if read_tx.send(result).is_err() { break; }
                }
                req = events_rx.recv() => {
                    let Some(req) = req else { break; };
                    let Some(a) = active else { continue; };
                    let result = load_contract_events_page::run(
                        &network_status,
                        &event_log,
                        a,
                        chain,
                        req.head_hint,
                        req.offset,
                    ).await;
                    if events_tx.send(result).is_err() { break; }
                }
                req = storage_rx.recv() => {
                    let Some(req) = req else { break; };
                    let Some(a) = active else { continue; };
                    let result = storage.get_at(a, chain, req.slot).await;
                    if storage_tx.send(result).is_err() { break; }
                }
            }
        }
    });
    Box::new(AddressDetailScreen::with_factories_and_tab(
        chain,
        address,
        feed,
        None,
        None,
        AddressTab::Contract,
    ))
}

/// Spawn a unified AddressDetail focused on the Token tab. Only
/// the token-reader port is wired so price / transfers / history
/// channels stay in the "loading..." state, matching the old
/// minimal `spawn_token_detail` helper. The address reader is
/// primed as well so the screen exposes the Contract tab next to
/// Token (required by plan/16 §2).
pub(crate) fn spawn_token_detail<
    R: TokenReaderPort + Clone + 'static,
    A: AddressReaderPort + Clone + 'static,
>(
    chain: Chain,
    address: Address,
    reader: R,
    address_reader: A,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    let (feed, sender) = address_feed();
    let reader_for_task = reader.clone();
    let address_reader_for_task = address_reader.clone();
    tokio::spawn(async move {
        let blockexplorer_tui::adapters::ui::AddressFeedSender {
            updates_tx,
            token_overview_tx,
            mut input_rx,
            ..
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            // Publish an Address overview (Contract) so Token +
            // Contract main tabs surface. When the address reader
            // has no entry, fall back to a synthesized Contract
            // overview so the screen can still reveal tabs.
            let overview = match address_reader_for_task.get(addr, chain).await {
                Ok(Some(ov)) => Some(ov),
                _ => None,
            };
            if let Some(ov) = overview
                && updates_tx.send(ov).is_err()
            {
                break;
            }
            match reader_for_task.get(addr, chain).await {
                Ok(Some(ov)) => {
                    if token_overview_tx.send(Some(ov)).is_err() {
                        break;
                    }
                }
                _ => {
                    let _ = token_overview_tx.send(None);
                }
            }
        }
    });
    Box::new(AddressDetailScreen::with_factories_and_tab(
        chain,
        address,
        feed,
        None,
        None,
        AddressTab::Token,
    ))
}

// `spawn_token_detail_with_full_feeds` was deleted alongside the
// `TokenDetailScreen` in Slice 3. Scenarios that exercised the
// full token feed migrated to `tests/e2e/features/address_detail.feature`
// backed by `spawn_address_detail_as_token_with_full_feeds` below.

pub(crate) fn spawn_address_detail<R: AddressReaderPort + Clone + 'static>(
    chain: Chain,
    address: Address,
    reader: R,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    let (feed, sender) = address_feed();
    let reader_for_task = reader.clone();
    tokio::spawn(async move {
        let blockexplorer_tui::adapters::ui::AddressFeedSender {
            updates_tx,
            mut input_rx,
            ..
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            if let Ok(Some(ov)) = reader_for_task.get(addr, chain).await
                && updates_tx.send(ov).is_err()
            {
                break;
            }
        }
    });
    Box::new(AddressDetailScreen::loading(chain, address, feed))
}

/// Address-detail spawner used by the Transactions-tab scenarios.
/// Reuses the real [`infra::address_feed::spawn`] wiring, so the
/// transfers stream also populates from the stub `TransfersPort`.
/// Wires Enter-on-a-transfer to open a TxDetail screen through the
/// supplied tx reader stub.
#[allow(dead_code)]
pub(crate) fn spawn_address_detail_with_transfers<
    R: AddressReaderPort + Clone + 'static,
    T: blockexplorer_tui::application::ports::TransfersPort + Clone + 'static,
    Tx: TxReaderPort + Clone + 'static,
>(
    chain: Chain,
    address: Address,
    reader: R,
    transfers: T,
    tx_reader: Tx,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    use blockexplorer_tui::adapters::ui::{AddressFeedSender, address_detail::OpenTxFactory};
    let (feed, sender) = address_feed();
    let reader_for_task = reader.clone();
    let transfers_for_task = transfers.clone();
    tokio::spawn(async move {
        let AddressFeedSender {
            updates_tx,
            transfers_tx,
            mut input_rx,
            ..
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            let (ov, page) = tokio::join!(
                reader_for_task.get(addr, chain),
                transfers_for_task.get_for_address(addr, chain, None),
            );
            if let Ok(Some(ov)) = ov
                && updates_tx.send(ov).is_err()
            {
                break;
            }
            if let Ok(page) = page
                && transfers_tx.send(page).is_err()
            {
                break;
            }
        }
    });

    let tx_reader_for_open = tx_reader.clone();
    let open_tx: OpenTxFactory =
        Box::new(move |hash| spawn_tx_detail(chain, hash, tx_reader_for_open.clone()));

    Box::new(
        blockexplorer_tui::adapters::ui::AddressDetailScreen::with_open_tx(
            chain,
            address,
            feed,
            Some(open_tx),
        ),
    )
}

/// Address-detail spawner wired up to overview + transfers +
/// portfolio stubs, plus an open_token factory that spawns a
/// TokenDetail screen through the stub token reader.
#[allow(dead_code)]
pub(crate) fn spawn_address_detail_with_full_feeds<
    R: AddressReaderPort + Clone + 'static,
    T: blockexplorer_tui::application::ports::TransfersPort + Clone + 'static,
    P: blockexplorer_tui::application::ports::PortfolioPort + Clone + 'static,
    Tx: TxReaderPort + Clone + 'static,
    Tok: blockexplorer_tui::application::ports::TokenReaderPort + Clone + 'static,
>(
    chain: Chain,
    address: Address,
    reader: R,
    transfers: T,
    portfolio: P,
    tx_reader: Tx,
    token_reader: Tok,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    use blockexplorer_tui::adapters::ui::{
        AddressFeedSender, OpenTokenFactory, address_detail::OpenTxFactory,
    };
    let (feed, sender) = address_feed();
    let reader_for_task = reader.clone();
    let transfers_for_task = transfers.clone();
    let portfolio_for_task = portfolio.clone();
    tokio::spawn(async move {
        let AddressFeedSender {
            updates_tx,
            transfers_tx,
            portfolio_tx,
            mut input_rx,
            ..
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            let (ov, page, holdings) = tokio::join!(
                reader_for_task.get(addr, chain),
                transfers_for_task.get_for_address(addr, chain, None),
                portfolio_for_task.get_token_balances(addr, chain),
            );
            if let Ok(Some(ov)) = ov
                && updates_tx.send(ov).is_err()
            {
                break;
            }
            if let Ok(page) = page
                && transfers_tx.send(page).is_err()
            {
                break;
            }
            if let Ok(holdings) = holdings
                && portfolio_tx.send(holdings).is_err()
            {
                break;
            }
        }
    });

    let tx_reader_for_open = tx_reader.clone();
    let open_tx: OpenTxFactory =
        Box::new(move |hash| spawn_tx_detail(chain, hash, tx_reader_for_open.clone()));

    let token_reader_for_open = token_reader.clone();
    let address_reader_for_open = reader.clone();
    let open_token: OpenTokenFactory = Box::new(move |contract| {
        spawn_token_detail(
            chain,
            contract,
            token_reader_for_open.clone(),
            address_reader_for_open.clone(),
        )
    });

    Box::new(
        blockexplorer_tui::adapters::ui::AddressDetailScreen::with_factories(
            chain,
            address,
            feed,
            Some(open_tx),
            Some(open_token),
        ),
    )
}

/// Address-detail spawner that overlays the reverse-ENS stub onto
/// the `AddressOverview` returned by the reader. Mirrors the
/// production composition (`load_address_overview` threads an
/// `EnsResolverPort` once plan/6 §11 "Shipped" lands) without
/// pulling the full feed.
#[allow(dead_code)]
pub(crate) fn spawn_address_detail_with_reverse_ens<
    R: AddressReaderPort + Clone + 'static,
    E: blockexplorer_tui::application::ports::EnsResolverPort + Clone + 'static,
>(
    chain: Chain,
    address: Address,
    reader: R,
    ens: E,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    let (feed, sender) = address_feed();
    let reader_for_task = reader.clone();
    let ens_for_task = ens.clone();
    tokio::spawn(async move {
        let blockexplorer_tui::adapters::ui::AddressFeedSender {
            updates_tx,
            mut input_rx,
            ..
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            let (ov_res, rev_res) = tokio::join!(
                reader_for_task.get(addr, chain),
                ens_for_task.reverse(addr, chain),
            );
            if let Ok(Some(mut ov)) = ov_res {
                if ov.ens_name.is_none()
                    && let Ok(Some(name)) = rev_res.as_ref()
                {
                    ov.ens_name = Some(name.clone());
                }
                if updates_tx.send(ov).is_err() {
                    break;
                }
            }
        }
    });
    Box::new(AddressDetailScreen::loading(chain, address, feed))
}

/// Address-detail spawner for the ERC-20 inline Token tab
/// scenarios. Runs a local spawn loop mirroring the `Contract /
/// IsToken` branch of `infra::address_feed::spawn`. Cheaper to
/// compose than the production path (no proxy / source / storage
/// ports needed for the inline tab assertions).
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn spawn_address_detail_with_erc20_probe<
    R: AddressReaderPort + Clone + Send + Sync + 'static,
    T: blockexplorer_tui::application::ports::TransfersPort + Clone + Send + Sync + 'static,
    P: blockexplorer_tui::application::ports::PortfolioPort + Clone + Send + Sync + 'static,
    Tx: TxReaderPort + Clone + Send + Sync + 'static,
    Tok: blockexplorer_tui::application::ports::TokenReaderPort + Clone + Send + Sync + 'static,
    Pr: blockexplorer_tui::application::ports::PricesPort + Clone + Send + Sync + 'static,
>(
    chain: Chain,
    address: Address,
    reader: R,
    transfers: T,
    portfolio: P,
    tx_reader: Tx,
    token_reader: Tok,
    prices: Pr,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    use blockexplorer_tui::adapters::ui::{OpenTokenFactory, address_detail::OpenTxFactory};
    use blockexplorer_tui::domain::PriceWindow;
    let (feed, sender) = address_feed();
    let reader_for_open = reader.clone();
    let token_reader_for_open = token_reader.clone();
    tokio::spawn(async move {
        let blockexplorer_tui::adapters::ui::AddressFeedSender {
            updates_tx,
            transfers_tx,
            portfolio_tx,
            token_overview_tx,
            token_price_tx,
            token_series_tx,
            token_transfers_tx,
            mut token_window_req_rx,
            mut input_rx,
            ..
        } = sender;
        let mut current_addr: Option<Address> = None;
        loop {
            tokio::select! {
                biased;
                maybe_addr = input_rx.recv() => {
                    let Some(addr) = maybe_addr else { break; };
                    current_addr = Some(addr);
                    let (ov_res, tr_res, pf_res) = tokio::join!(
                        reader.get(addr, chain),
                        transfers.get_for_address(addr, chain, None),
                        portfolio.get_token_balances(addr, chain),
                    );
                    let kind_contract = matches!(
                        ov_res.as_ref(),
                        Ok(Some(ov)) if matches!(
                            ov.kind,
                            blockexplorer_tui::domain::AddressKind::Contract
                        )
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
                    if !kind_contract {
                        continue;
                    }
                    match token_reader.get(addr, chain).await {
                        Ok(Some(tov)) => {
                            if token_overview_tx.send(Some(tov)).is_err() {
                                break;
                            }
                            let (price_res, series_res, token_tr_res) = tokio::join!(
                                prices.get_single(addr, chain),
                                prices.get_history(addr, chain, PriceWindow::D1),
                                transfers.get_for_contract(addr, chain, None),
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
                        }
                        _ => {
                            let _ = token_overview_tx.send(None);
                        }
                    }
                }
                maybe_window = token_window_req_rx.recv() => {
                    let Some(window) = maybe_window else { break; };
                    let Some(addr) = current_addr else { continue; };
                    if let Ok(series) = prices.get_history(addr, chain, window).await
                        && token_series_tx.send(series).is_err()
                    {
                        break;
                    }
                }
            }
        }
    });

    let tx_reader_for_open = tx_reader.clone();
    let open_tx: OpenTxFactory =
        Box::new(move |hash| spawn_tx_detail(chain, hash, tx_reader_for_open.clone()));
    let open_token: OpenTokenFactory = Box::new(move |contract| {
        spawn_token_detail(
            chain,
            contract,
            token_reader_for_open.clone(),
            reader_for_open.clone(),
        )
    });

    Box::new(
        blockexplorer_tui::adapters::ui::AddressDetailScreen::with_factories(
            chain,
            address,
            feed,
            Some(open_tx),
            Some(open_token),
        ),
    )
}

pub(crate) fn spawn_tx_detail<R: TxReaderPort + Clone + 'static>(
    chain: Chain,
    hash: TxHash,
    reader: R,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    let (feed, sender) = tx_feed();
    let reader_for_task = reader.clone();
    tokio::spawn(async move {
        let blockexplorer_tui::adapters::ui::TxFeedSender {
            updates_tx,
            mut input_rx,
        } = sender;
        while let Some(h) = input_rx.recv().await {
            if let Ok(Some(tx)) = reader_for_task.get(h, chain).await
                && updates_tx
                    .send(blockexplorer_tui::application::TxView::bare(tx))
                    .is_err()
            {
                break;
            }
        }
    });
    Box::new(TxDetailScreen::loading(chain, hash, feed))
}

fn press(stack: &mut ScreenStack, key: KeyEvent) {
    let cmd = stack.top_mut().expect("stack non-empty").handle_key(key);
    stack.apply_command(cmd);
}

fn tick(stack: &mut ScreenStack) {
    let cmd = stack.top_mut().expect("stack non-empty").tick();
    stack.apply_command(cmd);
}

async fn type_and_resolve(stack: &mut ScreenStack, input: &str) {
    // Press `/` from Home to open the Search screen.
    press(stack, KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    assert_eq!(stack.top().unwrap().title(), "Search");

    for c in input.chars() {
        press(stack, KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }

    // Give the resolver task a chance to run. The task awaits via the
    // tokio channel so a short sleep inside the async step is enough.
    // We tick repeatedly to drain the feed into the screen.
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        tick(stack);
        if !current_candidates(stack).is_empty() {
            break;
        }
    }
}

fn current_candidates(stack: &ScreenStack) -> Vec<ResolvedEntity> {
    let top = stack.top().expect("stack non-empty");
    let search = top
        .as_any()
        .downcast_ref::<SearchScreen>()
        .expect("top of stack must be a SearchScreen");
    search.candidates().to_vec()
}

// ---------------------------------------------------------------------------
// Background / setup
// ---------------------------------------------------------------------------

// `Given the user is on Home` and `Given the active chain is "…"` live
// in `steps::shared` alongside `Given the user launches the app`. The
// shared module delegates to `search::build_stack` for the actual
// stack construction, so the behaviour is identical.

// ---------------------------------------------------------------------------
// Given — stub priming
// ---------------------------------------------------------------------------

#[given(regex = r#"^the stub knows the transaction "(0x[0-9a-fA-F]{64})"$"#)]
async fn stub_knows_tx(world: &mut AppWorld, hash_hex: String) {
    let hash = TxHash::from_hex(&hash_hex).expect("valid hex");
    world.tx_stub.insert(TxSummary {
        hash,
        block: Some(BlockNumber::new(21_000_000)),
    });
}

#[given(
    regex = r#"^the hash "(0x[0-9a-fA-F]{64})" matches both a transaction and a block in the stub$"#
)]
async fn stub_knows_both(world: &mut AppWorld, hex: String) {
    let tx_hash = TxHash::from_hex(&hex).unwrap();
    let block_hash = BlockHash::from_hex(&hex).unwrap();
    world.tx_stub.insert(TxSummary {
        hash: tx_hash,
        block: Some(BlockNumber::new(21_000_000)),
    });
    world.block_stub.insert(BlockSummary {
        number: BlockNumber::new(21_000_000),
        hash: block_hash,
    });
}

#[given(regex = r#"^"([^"]+)" resolves to "(0x[0-9a-fA-F]{40})" in the stub$"#)]
async fn stub_ens_forward(world: &mut AppWorld, name: String, addr_hex: String) {
    let addr = Address::from_hex(&addr_hex).unwrap();
    world.ens_stub.set_forward(&name, addr);
    world
        .address_stub
        .set_kind(addr, AddressKind::Eoa { delegated_to: None });
}

#[given(regex = r#"^the stub knows block (\d+)$"#)]
async fn stub_knows_block(world: &mut AppWorld, number: u64) {
    let hash =
        BlockHash::from_hex("0xabcdef0000000000000000000000000000000000000000000000000000000000")
            .unwrap();
    world.block_stub.insert(BlockSummary {
        number: BlockNumber::new(number),
        hash,
    });
}

#[given(regex = r#"^the address stub classifies "(0x[0-9a-fA-F]{40})" as a contract$"#)]
async fn address_stub_contract(world: &mut AppWorld, addr_hex: String) {
    let addr = Address::from_hex(&addr_hex).unwrap();
    world.address_stub.set_kind(addr, AddressKind::Contract);
}

/// Classify an address from a raw `eth_getCode`-shaped hex string. The
/// scenario uses a Gherkin-friendly ellipsis (`…`) which we silently
/// truncate before parsing so the step remains readable.
#[given(regex = r#"^the address "(0x[0-9a-fA-F]{40})" has code "([^"]+)" on that chain$"#)]
async fn address_has_code(world: &mut AppWorld, addr_hex: String, code: String) {
    let addr = Address::from_hex(&addr_hex).unwrap();
    let body = code
        .strip_prefix("0x")
        .or_else(|| code.strip_prefix("0X"))
        .unwrap_or(&code);
    let body = body.trim_end_matches('…');
    let kind = if let Some(rest) = body
        .strip_prefix("ef0100")
        .or_else(|| body.strip_prefix("EF0100"))
    {
        // Pad or truncate the delegate to exactly 40 hex chars so the
        // scenario can keep the "…" shorthand and still exercise the
        // 7702 detection branch with a stable delegate address.
        let mut delegate_hex = String::with_capacity(42);
        delegate_hex.push_str("0x");
        let rest_chars: String = rest.chars().take(40).collect();
        delegate_hex.push_str(&rest_chars);
        for _ in rest_chars.len()..40 {
            delegate_hex.push('0');
        }
        let delegate = Address::from_hex(&delegate_hex).unwrap();
        AddressKind::Eoa {
            delegated_to: Some(delegate),
        }
    } else if body.is_empty() || body.chars().all(|c| c == '0') {
        AddressKind::Eoa { delegated_to: None }
    } else {
        AddressKind::Contract
    };
    world.address_stub.set_kind(addr, kind);
}

// ---------------------------------------------------------------------------
// When — interactions
// ---------------------------------------------------------------------------

#[when(regex = r#"^the user opens search with "([^"]+)"$"#)]
async fn opens_search_with(world: &mut AppWorld, input: String) {
    build_stack(world);
    let stack = world.stack.as_mut().expect("stack exists");
    type_and_resolve(stack, &input).await;
}

#[when(regex = r#"^the user searches for "([^"]+)"$"#)]
async fn user_searches_for(world: &mut AppWorld, input: String) {
    opens_search_with(world, input).await;
}

// ---------------------------------------------------------------------------
// Then — assertions
// ---------------------------------------------------------------------------

#[then(regex = r#"^the primary candidate is "(transaction|block|address|token|not found)"$"#)]
async fn primary_candidate_is(world: &mut AppWorld, kind: String) {
    let stack = world.stack.as_ref().expect("stack exists");
    let candidates = current_candidates(stack);
    let first = candidates.first().expect("at least one candidate");
    assert_eq!(first.kind_label(), kind);
}

#[then(regex = r#"^pressing Enter pushes the "([^"]+)" screen$"#)]
async fn enter_pushes_screen(world: &mut AppWorld, title: String) {
    let stack = world.stack.as_mut().expect("stack exists");
    press(stack, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(stack.top().unwrap().title(), title);
}

#[then("there are at least two candidates in the list")]
async fn at_least_two_candidates(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack exists");
    assert!(current_candidates(stack).len() >= 2);
}

#[then(regex = r#"^the first candidate is "(transaction|block|address|token|not found)"$"#)]
async fn first_candidate_is(world: &mut AppWorld, kind: String) {
    // Alias of `primary_candidate_is` used by the disambiguation
    // scenario. Delegates to the same logic.
    primary_candidate_is(world, kind).await;
}

#[then(regex = r#"^the candidate row shows the ENS name "([^"]+)"$"#)]
async fn candidate_shows_ens_name(world: &mut AppWorld, expected: String) {
    let stack = world.stack.as_ref().expect("stack exists");
    let first = current_candidates(stack)
        .into_iter()
        .next()
        .expect("candidate");
    match first {
        ResolvedEntity::Address { ens_name, .. } => {
            assert_eq!(ens_name.as_deref(), Some(expected.as_str()));
        }
        other => panic!("expected Address, got {other:?}"),
    }
}

#[then(regex = r#"^the candidates list contains a single "not found" entry$"#)]
async fn single_not_found(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack exists");
    let candidates = current_candidates(stack);
    assert_eq!(candidates.len(), 1);
    assert!(matches!(
        candidates.first(),
        Some(ResolvedEntity::NotFound { .. })
    ));
}

#[then(regex = r#"^the candidates include a "contract" entry for "(0x[0-9a-fA-F]{40})"$"#)]
async fn candidates_include_contract_for(world: &mut AppWorld, addr_hex: String) {
    let expected = Address::from_hex(&addr_hex).unwrap();
    let stack = world.stack.as_ref().expect("stack exists");
    let candidates = current_candidates(stack);
    assert!(
        candidates.iter().any(|c| matches!(
            c,
            ResolvedEntity::Contract { address } if *address == expected
        )),
        "no Contract shortcut for {addr_hex} in {candidates:?}",
    );
}

#[then(regex = r#"^once the ERC-20 probe completes, a "token" candidate "([^"]+)" is appended$"#)]
async fn erc20_probe_appends_token(world: &mut AppWorld, expected_symbol: String) {
    let stack = world.stack.as_mut().expect("stack exists");
    // The probe runs on a Tokio task after the base update; poll
    // the screen until the Token row lands (or the timeout expires).
    let mut landed = false;
    for _ in 0..50 {
        tick(stack);
        let cands = current_candidates(stack);
        if cands.iter().any(|c| {
            matches!(
                c,
                ResolvedEntity::Token(m) if m.symbol == expected_symbol
            )
        }) {
            landed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        landed,
        "Token {expected_symbol} never appeared after ERC-20 probe",
    );
}

#[then(regex = r#"^the primary candidate is still "(transaction|block|address|token|not found)"$"#)]
async fn primary_candidate_is_still(world: &mut AppWorld, kind: String) {
    primary_candidate_is(world, kind).await;
}

#[then("the first result is an Address row flagged as delegated")]
async fn first_result_is_delegated(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack exists");
    let candidates = current_candidates(stack);
    match candidates.first() {
        Some(ResolvedEntity::DelegatedEoa { .. }) => {}
        other => panic!("expected DelegatedEoa row first, got {other:?}"),
    }
}

#[then("the second result is not a Contract row")]
async fn second_result_not_contract(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack exists");
    let candidates = current_candidates(stack);
    match candidates.get(1) {
        None => {}
        Some(ResolvedEntity::Contract { .. }) => {
            panic!("expected no Contract shortcut on delegated-EOA, got one")
        }
        Some(_) => {}
    }
}

#[then("the hint mentions the active chain name")]
async fn hint_mentions_chain(world: &mut AppWorld) {
    let chain = world.active_chain.expect("chain");
    let stack = world.stack.as_ref().expect("stack exists");
    let candidates = current_candidates(stack);
    match candidates.first() {
        Some(ResolvedEntity::NotFound { reason }) => {
            assert!(
                reason.contains(chain.display_name()),
                "reason `{reason}` must mention {}",
                chain.display_name(),
            );
        }
        other => panic!("expected NotFound, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Search cache scenarios (plan/2 §12.4)
// ---------------------------------------------------------------------------

#[given(regex = r#"^the search cache is enabled with a (\d+) second TTL$"#)]
async fn enable_search_cache(world: &mut AppWorld, ttl_secs: u64) {
    use blockexplorer_tui::adapters::cache::TtlCache;

    let clock = crate::support::stubs::FrozenClock::default();
    world.search_cache = Some(TtlCache::with_ttl_and_clock(
        Duration::from_secs(ttl_secs),
        clock,
    ));
    // Drop the pre-cache stack so the next interaction rebuilds
    // HomeScreen with a search_factory that captures the fresh cache.
    world.stack = None;
}

#[when("the user closes the search modal")]
async fn close_search_modal(world: &mut AppWorld) {
    // Give the previous feed task a tick to flush the cache write
    // that happens right after it sends the last update. The write
    // is `await`ed in the task but lives on a separate tokio task,
    // so without the yield we occasionally race the close.
    tokio::time::sleep(Duration::from_millis(30)).await;
    let stack = world.stack.as_mut().expect("stack exists");
    press(stack, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(stack.top().unwrap().title(), "Home");
}

#[then(regex = r#"^the tx lookup stub was called exactly (\d+) times?$"#)]
async fn tx_lookup_call_count(world: &mut AppWorld, expected: usize) {
    assert_eq!(world.tx_stub.call_count(), expected, "tx lookup call count",);
}

// ---------------------------------------------------------------------------
// Overlay scenario (plan/2-search.md §13)
// ---------------------------------------------------------------------------

/// Composite render of the current stack into a `TestBackend` so
/// the overlay scenario can assert on the final buffer. Mirrors
/// `src/infra/runtime.rs::redraw`: top screen first at the full
/// area, then the modal at the same area.
fn render_stack_to_buffer(stack: &ScreenStack, width: u16, height: u16) -> ratatui::buffer::Buffer {
    use ratatui::{Terminal, backend::TestBackend};
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| {
            let area = frame.area();
            if let Some(top) = stack.top() {
                top.render(frame, area);
            }
            if let Some(modal) = stack.modal() {
                modal.render(frame, area);
            }
        })
        .expect("draw");
    terminal.backend().buffer().clone()
}

fn overlay_row_text(buffer: &ratatui::buffer::Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol().to_string())
        .collect()
}

fn overlay_buffer_contains(buffer: &ratatui::buffer::Buffer, needle: &str) -> bool {
    (0..buffer.area.height).any(|y| overlay_row_text(buffer, y).contains(needle))
}

fn overlay_row_of(buffer: &ratatui::buffer::Buffer, needle: &str) -> Option<u16> {
    (0..buffer.area.height).find(|y| overlay_row_text(buffer, *y).contains(needle))
}

/// Footer strip only — same as `tests/functional/search_overlay_render.rs`.
fn overlay_row_of_in_bottom_footer(buffer: &ratatui::buffer::Buffer, needle: &str) -> Option<u16> {
    let h = buffer.area.height;
    (h.saturating_sub(3)..h).find(|y| overlay_row_text(buffer, *y).contains(needle))
}

#[when(regex = r#"^the user opens the search overlay with "/"$"#)]
async fn user_opens_search_overlay(world: &mut AppWorld) {
    build_stack(world);
    let chain = ensure_active_chain(world);
    // Build a real SearchScreen through the search factory and
    // feed it into the stack's modal slot: this mirrors the
    // real runtime, where pressing `/` goes through
    // `GlobalKeyMap::dispatch` and returns `Command::OpenModal`.
    let factory = build_search_factory(world, chain);
    let stack = world.stack.as_mut().expect("stack exists");
    stack.apply_command(Command::OpenModal(factory()));
    assert_eq!(
        stack.top().unwrap().title(),
        "Home",
        "Home must remain the back-stack top when `/` opens a modal"
    );
    assert_eq!(
        stack.modal().unwrap().title(),
        "Search",
        "Search must live in the modal slot, not on the back stack"
    );
}

#[then("the Home screen is still rendered behind the overlay")]
async fn home_visible_behind_overlay(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack exists");
    let buffer = render_stack_to_buffer(stack, 120, 30);

    // Home's header shows the active chain. Search must not
    // paint over it.
    let chain = world.active_chain.expect("chain");
    let needle = format!("Chain: {}", chain.display_name());
    assert!(
        overlay_buffer_contains(&buffer, &needle),
        "Home header `{needle}` must survive the overlay; got:\n{}",
        (0..buffer.area.height)
            .map(|y| overlay_row_text(&buffer, y))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    // Home's top-left border corner must stay intact (it lives
    // well outside the centered overlay rectangle).
    let corner = buffer[(0, 0)].symbol().to_string();
    assert_eq!(
        corner, "┌",
        "Home's top-left border corner must stay intact (got {corner:?})",
    );
}

#[then("the search input is rendered at the bottom of the screen")]
async fn search_input_at_bottom(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack exists");
    let buffer = render_stack_to_buffer(stack, 120, 30);

    let prompt_row = overlay_row_of_in_bottom_footer(&buffer, "> ")
        .expect("search prompt (`> ` line editor) must render");
    assert!(
        prompt_row >= buffer.area.height - 3,
        "search prompt must sit inside the bottom 3 rows, got row {prompt_row} of {}",
        buffer.area.height,
    );
}

#[then("the results panel is rendered as a centered floating modal")]
async fn results_panel_is_centered(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack exists");
    let buffer = render_stack_to_buffer(stack, 120, 30);

    let row =
        overlay_row_of(&buffer, "Candidates").expect("Candidates border title must be rendered");
    assert!(
        row > 2 && row < buffer.area.height - 3,
        "Candidates border must live in the middle of the buffer, got row {row}",
    );
}
