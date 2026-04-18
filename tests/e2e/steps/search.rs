//! Step definitions for the Search feature.
//!
//! Each scenario drives the real SearchScreen/HomeScreen through the
//! stub ports held by the cucumber world. The scenarios live in
//! `tests/e2e/features/search.feature`; the underlying logic lives in
//! `plan/2-search.md` section 10.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{
        AddressDetailScreen, BlockDetailScreen, Command, ContractDetailScreen,
        DetailPlaceholderScreen, HomeScreen, ScreenStack, SearchScreen, TokenDetailScreen,
        TxDetailScreen, address_feed, block_feed, contract_feed, home_feed, search_feed,
        token_feed, tx_feed,
    },
    application::{
        ConnectionStatus, HomeViewModel,
        ports::{
            AddressReaderPort, BlockReaderPort, ProxyDetectionPort, TokenReaderPort,
            TxReaderPort,
        },
        use_cases::{load_contract_overview, resolve_query::ResolveQuery},
    },
    domain::{
        Address, AddressKind, BlockHash, BlockId, BlockNumber, BlockSummary, Chain,
        ResolvedEntity, TxHash, TxSummary,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use cucumber::{given, then, when};

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

    Box::new(move || {
        let (feed, sender) = search_feed();
        let block = block.clone();
        let tx = tx.clone();
        let address = address.clone();
        let ens = ens.clone();
        let token = token.clone();
        let block_reader_for_detail = block_reader.clone();
        let tx_reader_for_detail = tx_reader.clone();
        let address_reader_for_detail = address_reader.clone();
        let proxy_detector_for_detail = proxy_detector.clone();
        let token_reader_for_detail = token_reader.clone();

        tokio::spawn(async move {
            let blockexplorer_tui::adapters::ui::SearchFeedSender {
                updates_tx,
                mut input_rx,
            } = sender;
            let query = ResolveQuery {
                block: &block,
                tx: &tx,
                address: &address,
                ens: &ens,
                token: &token,
            };
            while let Some(input) = input_rx.recv().await {
                let trimmed = input.trim();
                let candidates = if trimmed.is_empty() {
                    Vec::new()
                } else {
                    query.run(trimmed, chain).await.unwrap_or_default()
                };
                if updates_tx
                    .send(blockexplorer_tui::adapters::ui::SearchFeedUpdate {
                        input: input.clone(),
                        candidates,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });

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
                ResolvedEntity::Tx { hash, .. } => {
                    spawn_tx_detail(chain, hash, tx_reader.clone())
                }
                ResolvedEntity::Address { address, kind, .. } => match kind {
                    AddressKind::Contract => spawn_contract_detail(
                        chain,
                        address,
                        address_reader.clone(),
                        proxy_detector.clone(),
                    ),
                    AddressKind::Eoa => {
                        spawn_address_detail(chain, address, address_reader.clone())
                    }
                },
                ResolvedEntity::Token(meta) => {
                    spawn_token_detail(chain, meta.address, token_reader.clone())
                }
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

pub(crate) fn spawn_contract_detail<
    A: AddressReaderPort + Clone + 'static,
    P: ProxyDetectionPort + Clone + 'static,
>(
    chain: Chain,
    address: Address,
    address_reader: A,
    proxy_detector: P,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    let (feed, sender) = contract_feed();
    let reader = address_reader.clone();
    let detector = proxy_detector.clone();
    tokio::spawn(async move {
        let blockexplorer_tui::adapters::ui::ContractFeedSender {
            updates_tx,
            source_tx: _,
            read_rx: _,
            read_tx: _,
            mut input_rx,
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            if let Ok(ov) =
                load_contract_overview::run(&reader, &detector, addr, chain).await
                && updates_tx.send(ov).is_err()
            {
                break;
            }
        }
    });
    Box::new(ContractDetailScreen::loading(chain, address, feed))
}

pub(crate) fn spawn_token_detail<R: TokenReaderPort + Clone + 'static>(
    chain: Chain,
    address: Address,
    reader: R,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    let (feed, sender) = token_feed();
    let reader_for_task = reader.clone();
    tokio::spawn(async move {
        let blockexplorer_tui::adapters::ui::TokenFeedSender {
            updates_tx,
            mut input_rx,
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            if let Ok(Some(ov)) = reader_for_task.get(addr, chain).await
                && updates_tx.send(ov).is_err()
            {
                break;
            }
        }
    });
    Box::new(TokenDetailScreen::loading(chain, address, feed))
}

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
            transfers_tx: _,
            portfolio_tx: _,
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
            portfolio_tx: _,
            mut input_rx,
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
    let open_tx: OpenTxFactory = Box::new(move |hash| {
        spawn_tx_detail(chain, hash, tx_reader_for_open.clone())
    });

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
    let open_tx: OpenTxFactory = Box::new(move |hash| {
        spawn_tx_detail(chain, hash, tx_reader_for_open.clone())
    });

    let token_reader_for_open = token_reader.clone();
    let open_token: OpenTokenFactory = Box::new(move |contract| {
        spawn_token_detail(chain, contract, token_reader_for_open.clone())
    });

    Box::new(
        blockexplorer_tui::adapters::ui::AddressDetailScreen::with_factories(
            chain,
            address,
            feed,
            Some(open_tx),
            Some(open_token),
            None,
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

/// Apply a [`Command`] returned by a screen to the world's stack. The
/// runtime's real dispatcher does the same (`src/infra/runtime.rs`).
fn apply_command(stack: &mut ScreenStack, cmd: Command) {
    match cmd {
        Command::None | Command::Refresh => {}
        Command::Pop => {
            stack.pop();
        }
        Command::Quit => stack.clear(),
        Command::Push(screen) => stack.push(screen),
        Command::Replace(screen) => {
            stack.pop();
            stack.push(screen);
        }
    }
}

fn press(stack: &mut ScreenStack, key: KeyEvent) {
    let cmd = stack
        .top_mut()
        .expect("stack non-empty")
        .handle_key(key);
    apply_command(stack, cmd);
}

fn tick(stack: &mut ScreenStack) {
    let cmd = stack.top_mut().expect("stack non-empty").tick();
    apply_command(stack, cmd);
}

async fn type_and_resolve(stack: &mut ScreenStack, input: &str) {
    // Press `/` from Home to open the Search screen.
    press(
        stack,
        KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
    );
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

#[given("the user is on Home")]
async fn user_on_home(world: &mut AppWorld) {
    build_stack(world);
}

// `Given the active chain is "..."` is already registered in
// steps/home.rs; cucumber picks the first match across modules so we
// do not re-register it here.

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
    world.address_stub.set_kind(addr, AddressKind::Eoa);
}

#[given(regex = r#"^the stub knows block (\d+)$"#)]
async fn stub_knows_block(world: &mut AppWorld, number: u64) {
    let hash = BlockHash::from_hex(
        "0xabcdef0000000000000000000000000000000000000000000000000000000000",
    )
    .unwrap();
    world.block_stub.insert(BlockSummary {
        number: BlockNumber::new(number),
        hash,
    });
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
