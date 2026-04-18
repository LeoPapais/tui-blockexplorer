//! Step definitions for the Mempool feature.
//!
//! See `plan/5-mempool.md` section 11.2. Scenarios live in
//! `tests/e2e/features/mempool.feature`.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{Command, MempoolScreen, ScreenStack},
    application::{ConnectionStatus, use_cases::observe_pending_txs::run as subscribe},
    domain::{Address, Chain, PendingTx, PendingTxFilter, TxHash, Wei},
    infra::mempool_feed::spawn_filter_drain,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use cucumber::{given, then, when};
use pretty_assertions::assert_eq;

use crate::{steps::search::build_stack, world::AppWorld};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ensure_active_chain(world: &mut AppWorld) -> Chain {
    *world.active_chain.get_or_insert(Chain::Ethereum)
}

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
    let cmd = stack.top_mut().expect("stack non-empty").handle_key(key);
    apply_command(stack, cmd);
}

async fn tick_and_wait(stack: &mut ScreenStack) {
    let cmd = stack.top_mut().expect("stack non-empty").tick();
    apply_command(stack, cmd);
    tokio::time::sleep(Duration::from_millis(5)).await;
}

async fn tick_until<F>(stack: &mut ScreenStack, mut predicate: F)
where
    F: FnMut(&ScreenStack) -> bool,
{
    for _ in 0..50 {
        tick_and_wait(stack).await;
        if predicate(stack) {
            return;
        }
    }
}

fn current_mempool(stack: &ScreenStack) -> &MempoolScreen {
    stack
        .top()
        .expect("stack non-empty")
        .as_any()
        .downcast_ref::<MempoolScreen>()
        .expect("top of stack must be a MempoolScreen")
}

fn current_mempool_mut(stack: &mut ScreenStack) -> &mut MempoolScreen {
    stack
        .top_mut()
        .expect("stack non-empty")
        .as_any_mut()
        .downcast_mut::<MempoolScreen>()
        .expect("top of stack must be a MempoolScreen")
}

async fn open_mempool(world: &mut AppWorld) {
    build_stack(world);
    let chain = ensure_active_chain(world);
    let port = world.pending_stub.clone();
    let rx = subscribe(&port, chain, PendingTxFilter::default())
        .await
        .expect("stub always subscribes");

    // Mirror the production wiring: the screen broadcasts filter
    // changes on a control channel; the infra task calls
    // `PendingTxStreamPort::update_filter`. This exercises §11.3.2
    // end-to-end — `set_filter` in BDD reaches the stub's history.
    let (filter_control, _drain_handle) = spawn_filter_drain(port.clone(), chain);

    // The open-tx factory is irrelevant for the current scenarios;
    // use a placeholder that panics if Enter is pressed unexpectedly.
    let open_tx: Box<dyn Fn(TxHash) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> + Send> =
        Box::new(|_| panic!("tx navigation not exercised by these scenarios"));

    let stack = world.stack.as_mut().unwrap();
    stack.push(Box::new(
        MempoolScreen::new(rx, PendingTxFilter::default(), open_tx)
            .with_filter_control(filter_control),
    ));
}

fn sample_pending(hash_hex: &str, from_hex: &str) -> PendingTx {
    PendingTx {
        hash: TxHash::from_hex(hash_hex).unwrap(),
        from: Address::from_hex(from_hex).unwrap(),
        to: Some(Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()),
        value: Wei::new(1_000),
    }
}

// ---------------------------------------------------------------------------
// Given
// ---------------------------------------------------------------------------

#[given("the user is on Mempool")]
async fn user_is_on_mempool(world: &mut AppWorld) {
    open_mempool(world).await;
}

#[given("the stub emits three pending txs from different senders")]
async fn stub_emits_three_different_senders(world: &mut AppWorld) {
    world.pending_stub.push_added(sample_pending(
        "0x1111000000000000000000000000000000000000000000000000000000000001",
        "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    ));
    world.pending_stub.push_added(sample_pending(
        "0x1111000000000000000000000000000000000000000000000000000000000002",
        "0x1111111111111111111111111111111111111111",
    ));
    world.pending_stub.push_added(sample_pending(
        "0x1111000000000000000000000000000000000000000000000000000000000003",
        "0x2222222222222222222222222222222222222222",
    ));
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current_mempool(s).items().len() == 3).await;
}

#[given(regex = r#"^the stub emits one pending tx with hash "(0x[0-9a-fA-F]{64})"$"#)]
async fn stub_emits_pending_with_hash(world: &mut AppWorld, hash_hex: String) {
    world.pending_stub.push_added(sample_pending(
        &hash_hex,
        "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    ));
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current_mempool(s).items().len() == 1).await;
}

// ---------------------------------------------------------------------------
// When
// ---------------------------------------------------------------------------

#[when("the stub emits three pending txs")]
async fn stub_emits_three(world: &mut AppWorld) {
    stub_emits_three_different_senders(world).await;
}

#[when("the stub emits one pending tx")]
async fn stub_emits_one(world: &mut AppWorld) {
    world.pending_stub.push_added(sample_pending(
        "0x7777000000000000000000000000000000000000000000000000000000000001",
        "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    ));
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current_mempool(s).items().len() == 1).await;
}

#[when("the stub emits two more pending txs")]
async fn stub_emits_two_more(world: &mut AppWorld) {
    world.pending_stub.push_added(sample_pending(
        "0x7777000000000000000000000000000000000000000000000000000000000002",
        "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    ));
    world.pending_stub.push_added(sample_pending(
        "0x7777000000000000000000000000000000000000000000000000000000000003",
        "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    ));
}

#[when(regex = r#"^the filter is set to from "(0x[0-9a-fA-F]{40})"$"#)]
async fn set_filter_from(world: &mut AppWorld, addr_hex: String) {
    let stack = world.stack.as_mut().expect("stack");
    let addr = Address::from_hex(&addr_hex).unwrap();
    current_mempool_mut(stack).set_filter(PendingTxFilter { from: Some(addr) });
}

#[when(regex = r#"^the user presses "(p)"$"#)]
async fn press_letter(world: &mut AppWorld, letter: String) {
    let stack = world.stack.as_mut().expect("stack");
    let c = letter.chars().next().unwrap();
    press(stack, KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
}

#[when(regex = r#"^the stub emits a removed event for "(0x[0-9a-fA-F]{64})"$"#)]
async fn stub_emits_removed(world: &mut AppWorld, hash_hex: String) {
    let hash = TxHash::from_hex(&hash_hex).unwrap();
    world.pending_stub.push_removed(hash);
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current_mempool(s).items().is_empty()).await;
}

#[when("the pending-tx stream drops")]
async fn stream_drops(world: &mut AppWorld) {
    world.pending_stub.disconnect_all();
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        matches!(
            current_mempool(s).stream_state(),
            ConnectionStatus::Disconnected { .. }
        )
    })
    .await;
}

// ---------------------------------------------------------------------------
// Then
// ---------------------------------------------------------------------------

#[then("three rows appear in the list")]
async fn three_rows(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current_mempool(s).items().len() == 3).await;
    assert_eq!(current_mempool(stack).items().len(), 3);
}

#[then("only the matching tx remains in the list")]
async fn only_matching_tx(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack");
    let items = current_mempool(stack).items();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].from.to_hex(),
        "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    );
}

#[then("the list still has one row")]
async fn still_one_row(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    // A paused screen should not absorb events even if we tick a few
    // times. Run a handful of tick cycles and confirm the count
    // stays at 1.
    for _ in 0..10 {
        tick_and_wait(stack).await;
    }
    assert_eq!(current_mempool(stack).items().len(), 1);
    assert!(current_mempool(stack).is_paused());
}

#[then("the list has three rows")]
async fn list_has_three(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current_mempool(s).items().len() == 3).await;
    assert_eq!(current_mempool(stack).items().len(), 3);
}

#[then("the list is empty")]
async fn list_is_empty(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack");
    assert!(current_mempool(stack).items().is_empty());
}

#[then("the mempool filter is empty")]
async fn filter_is_empty(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack");
    assert_eq!(current_mempool(stack).filter(), &PendingTxFilter::default());
}

#[then("the reconnecting badge is not shown")]
async fn reconnect_badge_absent(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack");
    assert_eq!(
        current_mempool(stack).stream_state(),
        &ConnectionStatus::Connected,
    );
}

#[then("the reconnecting badge is shown")]
async fn reconnect_badge_present(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack");
    assert_eq!(
        current_mempool(stack).stream_state(),
        &ConnectionStatus::Disconnected {
            reconnect_scheduled: true
        },
    );
}

#[then("three rows still appear in the list")]
async fn three_rows_still(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack");
    assert_eq!(current_mempool(stack).items().len(), 3);
}

#[then(regex = r#"^the port recorded an update_filter from "(0x[0-9a-fA-F]{40})"$"#)]
async fn port_recorded_filter(world: &mut AppWorld, addr_hex: String) {
    let expected = Address::from_hex(&addr_hex).unwrap();
    // The filter-drain task runs on the Tokio executor; give it a few
    // short yields to consume the control-channel message before we
    // read the stub history.
    let history = tokio::time::timeout(Duration::from_millis(200), async {
        loop {
            let history = world.pending_stub.filter_history();
            if history
                .iter()
                .any(|(_, f)| f.from == Some(expected))
            {
                break history;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("filter update reached the port within 200ms");

    let last = history
        .last()
        .expect("at least one recorded filter update");
    assert_eq!(last.1.from, Some(expected));
}
