//! Step definitions for the Home screen.
//!
//! Scenarios live in `tests/e2e/features/home.feature`. Behaviour is
//! specified in `plan/1-home.md`.
//!
//! Each step drives the real application types through the stub ports
//! held by the [`AppWorld`] so the assertions in the `Then` steps exercise
//! the same code paths the UI adapter will use.

use blockexplorer_tui::{
    adapters::ui::{HomeScreen, Screen, home},
    application::{ConnectionStatus, HomeSession, HomeViewModel, use_cases::observe_new_heads},
    domain::{BlockNumber, Chain, NewHead},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use cucumber::{given, then, when};
use pretty_assertions::assert_eq;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

use crate::support::stubs::{GasSnapshotFixture, NetworkStatusFixture};
use crate::world::AppWorld;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn prime_fixtures_for(world: &AppWorld, chain: Chain) {
    let (net_fixture, gas_fixture) = fixtures_for(chain);
    world
        .network_stub
        .set_snapshot(NetworkStatusFixture::load(net_fixture));
    world
        .gas_stub
        .set_snapshot(GasSnapshotFixture::load(gas_fixture));
}

/// Public re-export so the shared `Given the active chain is "…"` step
/// in `steps::shared` can prime the Home fixtures without duplicating
/// the per-chain filename mapping.
pub(crate) fn prime_home_fixtures_for(world: &AppWorld, chain: Chain) {
    prime_fixtures_for(world, chain);
}

fn fixtures_for(chain: Chain) -> (&'static str, &'static str) {
    match chain {
        Chain::Ethereum => (
            "home__network_status__ethereum.json",
            "home__gas_snapshot__ethereum.json",
        ),
        Chain::Base => (
            "home__network_status__base.json",
            "home__gas_snapshot__base.json",
        ),
        Chain::Polygon => (
            "home__network_status__polygon.json",
            "home__gas_snapshot__polygon.json",
        ),
        other => panic!("no fixture primed for chain {}", other.slug()),
    }
}

fn start_home(world: &mut AppWorld, chain: Chain) {
    prime_fixtures_for(world, chain);
    world.active_chain = Some(chain);

    let mut session = HomeSession::new(
        world.network_stub.clone(),
        world.gas_stub.clone(),
        world.chain_registry.clone(),
        chain,
    );
    // Drive the first refresh synchronously so the view model is ready
    // for later steps. Cucumber steps are async, but we can block on the
    // futures because the stubs never suspend.
    futures_lite_block_on(async {
        session
            .refresh()
            .await
            .expect("initial refresh must succeed");
    });
    world.home = Some(session);
}

/// Small inline block_on that does not require an extra crate. The stubs
/// are fully synchronous so the future completes in the first poll.
fn futures_lite_block_on<F: std::future::Future>(fut: F) -> F::Output {
    use std::{
        pin::pin,
        task::{Context, Poll, Waker},
    };

    let mut fut = pin!(fut);
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    match fut.as_mut().poll(&mut cx) {
        Poll::Ready(value) => value,
        Poll::Pending => {
            panic!("stub future returned Pending; the stubs are expected to resolve immediately")
        }
    }
}

// ---------------------------------------------------------------------------
// Given
// ---------------------------------------------------------------------------

// `Given the user launches the app` and `Given the active chain is "…"`
// are registered in `steps::shared` because every feature file relies
// on them; we no longer duplicate them here.

#[given("the Home screen is rendered")]
async fn given_home_is_rendered(world: &mut AppWorld) {
    let chain = world
        .active_chain
        .expect("active chain must be set in Background");
    start_home(world, chain);
}

#[given(regex = r#"^the Home screen is rendered with "([^"]+)"$"#)]
async fn home_is_rendered_with(world: &mut AppWorld, chain: String) {
    let chain = Chain::from_slug(&chain).expect("scenario references a known chain");
    start_home(world, chain);
}

// ---------------------------------------------------------------------------
// When
// ---------------------------------------------------------------------------

#[when("the Home screen is rendered")]
async fn when_home_is_rendered(world: &mut AppWorld) {
    let chain = world
        .active_chain
        .expect("active chain must be set in Background");
    start_home(world, chain);
}

#[when(regex = r#"^a new "newHeads" event is pushed from the stub$"#)]
async fn newheads_pushed(world: &mut AppWorld) {
    // Swap the primed fixtures for the "next head" variants and let the
    // session re-read them.
    world.network_stub.set_snapshot(NetworkStatusFixture::load(
        "home__network_status__ethereum_next_head.json",
    ));
    world.gas_stub.set_snapshot(GasSnapshotFixture::load(
        "home__gas_snapshot__ethereum_next_head.json",
    ));
    let session = world.home.as_mut().expect("session must exist");
    futures_lite_block_on(async {
        session
            .on_new_head()
            .await
            .expect("new head refresh must succeed");
    });
}

#[when(regex = r#"^the user presses "c"$"#)]
async fn user_presses_c(_world: &mut AppWorld) {
    // The chain picker is a UI-level interaction that opens a modal.
    // From the application layer's point of view there is nothing to do
    // until the user makes a selection in the next step.
}

#[when(regex = r#"^selects "([^"]+)"$"#)]
async fn selects_chain(world: &mut AppWorld, chain: String) {
    let target = Chain::from_slug(&chain).expect("scenario references a known chain");
    prime_fixtures_for(world, target);
    let session = world.home.as_mut().expect("session must exist");
    futures_lite_block_on(async {
        session
            .switch_chain(target)
            .await
            .expect("switching to an enabled chain must succeed");
    });
    world.active_chain = Some(target);
}

#[when(regex = r#"^the "newHeads" subscription drops$"#)]
async fn subscription_drops(world: &mut AppWorld) {
    world.network_stub.set_broken(true);
    let session = world.home.as_mut().expect("session must exist");
    session.on_connection_drop();
}

#[when(regex = r#"^a new head is received from the "newHeads" subscription$"#)]
async fn new_head_received(world: &mut AppWorld) {
    // Subscribe via the stub port, push a head with the "next head"
    // number and drive it through HomeSession::on_new_head_event. The
    // network / gas stubs are re-primed with the "next head" fixtures
    // so the refresh picks up the new values, matching the contract
    // described in plan/1-home.md §12.3.
    let chain = world.active_chain.expect("active chain must be set");
    let mut rx = futures_lite_block_on(async {
        observe_new_heads::run(&world.new_heads_stub, chain)
            .await
            .expect("subscribe ok")
    });

    world.network_stub.set_snapshot(NetworkStatusFixture::load(
        "home__network_status__ethereum_next_head.json",
    ));
    world.gas_stub.set_snapshot(GasSnapshotFixture::load(
        "home__gas_snapshot__ethereum_next_head.json",
    ));

    let head = NewHead {
        chain,
        number: BlockNumber::new(21_345_679),
    };
    world.new_heads_stub.push_head(head);

    let received = rx.try_recv().expect("head must land in the channel");
    assert_eq!(received, head);

    let session = world.home.as_mut().expect("session must exist");
    futures_lite_block_on(async {
        session
            .on_new_head_event(received)
            .await
            .expect("refresh ok");
    });
}

// ---------------------------------------------------------------------------
// Then
// ---------------------------------------------------------------------------

#[then("the Network card shows the latest block number from the stub")]
async fn network_card_shows_block_number(world: &mut AppWorld) {
    let chain = world.active_chain.expect("active chain");
    let expected = world.network_stub.expected(chain);
    let actual = world
        .home
        .as_ref()
        .and_then(|s| s.view().network.clone())
        .expect("network status must be populated");
    assert_eq!(actual.latest_block, expected.latest_block);
}

#[then("the Gas card shows slow, average and fast gwei values")]
async fn gas_card_shows_values(world: &mut AppWorld) {
    let gas = world
        .home
        .as_ref()
        .and_then(|s| s.view().gas.clone())
        .expect("gas snapshot must be populated");
    assert!(gas.slow.value() > 0 || gas.average.value() > 0 || gas.fast.value() > 0);
    assert!(!gas.trend.is_empty());
}

#[then("the Network card updates the latest block number")]
async fn network_card_updates(world: &mut AppWorld) {
    let view = world.home.as_ref().expect("session").view();
    assert_eq!(
        view.network.as_ref().unwrap().latest_block.value(),
        21_345_679
    );
}

#[then("the Gas card recomputes its values")]
async fn gas_card_recomputes(world: &mut AppWorld) {
    let view = world.home.as_ref().expect("session").view();
    let gas = view.gas.as_ref().expect("gas snapshot");
    assert_eq!(gas.average.value(), 17);
    assert_eq!(gas.fast.value(), 22);
}

#[then(regex = r#"^the active chain becomes "([^"]+)"$"#)]
async fn active_chain_becomes(world: &mut AppWorld, chain: String) {
    let expected = Chain::from_slug(&chain).expect("known chain slug");
    let view = world.home.as_ref().expect("session").view();
    assert_eq!(view.chain, expected);
}

#[then(regex = r#"^the Network card reflects the latest block number for "([^"]+)"$"#)]
async fn network_card_reflects_chain(world: &mut AppWorld, chain: String) {
    let chain = Chain::from_slug(&chain).expect("known chain slug");
    let expected = world.network_stub.expected(chain);
    let view = world.home.as_ref().expect("session").view();
    let actual = view.network.as_ref().expect("network status");
    assert_eq!(actual.chain, chain);
    assert_eq!(actual.latest_block, expected.latest_block);
}

#[then(regex = r#"^the header shows a "disconnected" badge$"#)]
async fn header_shows_disconnected(world: &mut AppWorld) {
    let view = world.home.as_ref().expect("session").view();
    assert!(matches!(
        view.connection,
        ConnectionStatus::Disconnected { .. }
    ));
}

#[then("the app schedules a reconnect")]
async fn app_schedules_reconnect(world: &mut AppWorld) {
    let view = world.home.as_ref().expect("session").view();
    match view.connection {
        ConnectionStatus::Disconnected {
            reconnect_scheduled,
        } => {
            assert!(reconnect_scheduled, "reconnect must be scheduled");
        }
        ConnectionStatus::Connected => panic!("expected disconnected state"),
    }
}

#[then(regex = r#"^the header shows a "reconnecting" hint$"#)]
async fn header_shows_reconnecting_hint(world: &mut AppWorld) {
    let buffer = render_home(world);
    assert!(
        buffer_contains(&buffer, "reconnecting"),
        "the rendered frame must display the reconnecting hint"
    );
}

#[then("the Network card still renders the last-known latest block")]
async fn network_card_renders_last_known_block(world: &mut AppWorld) {
    let view = world.home.as_ref().expect("session").view();
    let network = view
        .network
        .as_ref()
        .expect("last-known network snapshot must survive the drop");
    let buffer = render_home(world);
    let rendered = format_u64(network.latest_block.value());
    assert!(
        buffer_contains(&buffer, &rendered),
        "the Network card must still show {rendered} while reconnecting"
    );
}

#[then("the Gas card still renders the last-known slow, average and fast gwei")]
async fn gas_card_renders_last_known_tiers(world: &mut AppWorld) {
    let view = world.home.as_ref().expect("session").view();
    let gas = view
        .gas
        .as_ref()
        .expect("last-known gas snapshot must survive the drop");
    let buffer = render_home(world);
    assert!(
        buffer_contains(&buffer, "Slow"),
        "the Gas card must still render the Slow tier"
    );
    assert!(
        buffer_contains(&buffer, "Avg"),
        "the Gas card must still render the Avg tier"
    );
    assert!(
        buffer_contains(&buffer, "Fast"),
        "the Gas card must still render the Fast tier"
    );

    let slow = gas.slow.value().to_string();
    let avg = gas.average.value().to_string();
    let fast = gas.fast.value().to_string();
    assert!(
        buffer_contains(&buffer, &slow)
            && buffer_contains(&buffer, &avg)
            && buffer_contains(&buffer, &fast),
        "all three gwei values ({slow}, {avg}, {fast}) must still render"
    );
}

// ---------------------------------------------------------------------------
// First-run banner (plan/10-settings.md §12.2)
// ---------------------------------------------------------------------------

fn render_home_screen(screen: &HomeScreen) -> Buffer {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| screen.render(frame, frame.area()))
        .expect("draw");
    terminal.backend().buffer().clone()
}

#[given("the Home screen is rendered without an alchemy key")]
async fn home_rendered_without_alchemy_key(world: &mut AppWorld) {
    let chain = world.active_chain.get_or_insert(Chain::Ethereum);
    let view = HomeViewModel {
        chain: *chain,
        network: None,
        gas: None,
        connection: ConnectionStatus::Connected,
    };
    world.home_screen = Some(HomeScreen::new(view).with_first_run_hint(true));
}

#[then("the Home screen shows the first-run credentials banner")]
async fn banner_is_visible(world: &mut AppWorld) {
    let screen = world
        .home_screen
        .as_ref()
        .expect("scenario primed a HomeScreen");
    let buffer = render_home_screen(screen);
    assert!(
        buffer_contains(&buffer, "Set up credentials"),
        "first-run banner must be visible"
    );
}

#[when("the user dismisses the first-run banner with Esc")]
async fn user_dismisses_banner_with_esc(world: &mut AppWorld) {
    let screen = world
        .home_screen
        .as_mut()
        .expect("scenario primed a HomeScreen");
    screen.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
}

#[then("the Home screen no longer shows the first-run credentials banner")]
async fn banner_is_gone(world: &mut AppWorld) {
    let screen = world
        .home_screen
        .as_ref()
        .expect("scenario primed a HomeScreen");
    let buffer = render_home_screen(screen);
    assert!(
        !buffer_contains(&buffer, "Set up credentials"),
        "first-run banner must disappear after Esc"
    );
}

fn render_home(world: &AppWorld) -> Buffer {
    let view: HomeViewModel = world
        .home
        .as_ref()
        .expect("session must exist before rendering")
        .view()
        .clone();
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| home::render(frame, frame.area(), &view))
        .expect("draw");
    terminal.backend().buffer().clone()
}

fn buffer_contains(buffer: &Buffer, needle: &str) -> bool {
    let mut row = String::new();
    for y in 0..buffer.area.height {
        row.clear();
        for x in 0..buffer.area.width {
            row.push_str(buffer[(x, y)].symbol());
        }
        if row.contains(needle) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Esc / q on Home (plan/12-screen-runtime.md §7.1)
// ---------------------------------------------------------------------------

fn dispatch_key_on_top(world: &mut AppWorld, key: KeyEvent) {
    let stack = world
        .stack
        .as_mut()
        .expect("Background `Given the user is on Home` must build the stack");
    let cmd = stack
        .top_mut()
        .expect("stack must have a top screen")
        .handle_key(key);
    let transition = stack.apply_command(cmd);
    if transition.should_exit() {
        world.stack_exited = true;
    }
}

#[when("the user presses Esc on Home")]
async fn user_presses_esc_on_home(world: &mut AppWorld) {
    dispatch_key_on_top(world, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
}

#[when("the user presses q on Home")]
async fn user_presses_q_on_home(world: &mut AppWorld) {
    dispatch_key_on_top(world, KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
}

#[then("Home is still on top of the stack")]
async fn home_is_still_on_top(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack must exist");
    let top = stack
        .top()
        .expect("stack must not be empty after Esc on Home");
    assert_eq!(top.title(), "Home");
    assert_eq!(stack.len(), 1);
}

#[then("the app is still running")]
async fn app_is_still_running(world: &mut AppWorld) {
    assert!(
        !world.stack_exited,
        "Pop on a single-screen stack must not signal Transition::Exit"
    );
}

#[then("the app is no longer running")]
async fn app_is_no_longer_running(world: &mut AppWorld) {
    assert!(
        world.stack_exited,
        "Quit must signal Transition::Exit and tear the event loop down"
    );
}

/// Mirrors `home::format_u64` (private). Keeps the step definitions
/// independent from UI internals while still asserting on the same
/// formatted output the user sees.
fn format_u64(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*byte as char);
    }
    out
}
