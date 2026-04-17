//! Step definitions for the Search feature.
//!
//! Each scenario drives the real SearchScreen/HomeScreen through the
//! stub ports held by the cucumber world. The scenarios live in
//! `tests/e2e/features/search.feature`; the underlying logic lives in
//! `plan/2-search.md` section 10.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{
        Command, DetailPlaceholderScreen, HomeScreen, ScreenStack, SearchScreen,
        home_feed, search_feed,
    },
    application::{ConnectionStatus, HomeViewModel, use_cases::resolve_query::ResolveQuery},
    domain::{
        Address, AddressKind, BlockHash, BlockNumber, BlockSummary, Chain, ResolvedEntity,
        TxHash, TxSummary,
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

fn build_stack(world: &mut AppWorld) {
    if world.stack.is_some() {
        return;
    }
    let chain = ensure_active_chain(world);

    // Home screen without a live feed; we only need it as the first
    // element on the stack. The search factory captures clones of the
    // stubs so pressing `/` produces a usable SearchScreen.
    let (home_feed_rx, _home_feed_tx) = home_feed();

    let block = world.block_stub.clone();
    let tx = world.tx_stub.clone();
    let address = world.address_stub.clone();
    let ens = world.ens_stub.clone();
    let token = world.token_stub.clone();

    let search_factory = Box::new(move || -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
        let (feed, sender) = search_feed();
        // Spawn a per-SearchScreen resolver task using the stub ports.
        let block = block.clone();
        let tx = tx.clone();
        let address = address.clone();
        let ens = ens.clone();
        let token = token.clone();
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
        Box::new(SearchScreen::new(
            feed,
            Box::new(|entity| Box::new(DetailPlaceholderScreen::new(entity))),
        ))
    });

    let home = HomeScreen::with_feed(initial_home_view(chain), home_feed_rx)
        .with_search_factory(search_factory);

    let mut stack = ScreenStack::new();
    stack.push(Box::new(home));
    world.stack = Some(stack);
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
