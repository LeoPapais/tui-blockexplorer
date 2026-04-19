//! Step definitions for the Gas Tracker feature.
//!
//! Scenarios cover `plan/9-gas-tracker.md` §11.1 (MVP) plus the §11.2,
//! §11.3 and §11.4 follow-ups shipped out of `plan/15-backlog.md`
//! §8.10.

use blockexplorer_tui::{
    adapters::ui::{
        GasTrackerScreen, Screen, ScreenStack,
        gas_tracker::{gas_feed, gas_refresh_channel},
    },
    domain::{Chain, GasSnapshot, Gwei},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use cucumber::{given, then, when};
use pretty_assertions::assert_eq;

use crate::{steps::search::build_stack, world::AppWorld};

fn current(stack: &ScreenStack) -> &GasTrackerScreen {
    stack
        .top()
        .expect("stack non-empty")
        .as_any()
        .downcast_ref::<GasTrackerScreen>()
        .expect("top of stack must be a GasTrackerScreen")
}

fn current_mut(stack: &mut ScreenStack) -> &mut GasTrackerScreen {
    stack
        .top_mut()
        .expect("stack non-empty")
        .as_any_mut()
        .downcast_mut::<GasTrackerScreen>()
        .expect("top of stack must be a GasTrackerScreen")
}

fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    }
}

fn snapshot_with_base(chain: Chain, base: u128) -> GasSnapshot {
    GasSnapshot {
        chain,
        slow: Gwei::new(base + 1),
        average: Gwei::new(base + 2),
        fast: Gwei::new(base + 3),
        base_fee: Gwei::new(base),
        trend: vec![Gwei::new(base); 20],
    }
}

#[given(regex = r#"^the gas stub snapshot has slow (\d+) average (\d+) fast (\d+)$"#)]
async fn gas_stub_snapshot(world: &mut AppWorld, slow: u128, average: u128, fast: u128) {
    let chain = *world.active_chain.get_or_insert(Chain::Ethereum);
    let snapshot = GasSnapshot {
        chain,
        slow: Gwei::new(slow),
        average: Gwei::new(average),
        fast: Gwei::new(fast),
        base_fee: Gwei::new(11),
        trend: vec![Gwei::new(11); 20],
    };
    world.gas_stub.set_snapshot(snapshot);
}

#[given("the user opens the Gas Tracker")]
#[when("the user opens the Gas Tracker")]
async fn opens_gas_tracker(world: &mut AppWorld) {
    build_stack(world);
    let chain = *world.active_chain.get_or_insert(Chain::Ethereum);
    let initial = Some(world.gas_stub.expected(chain));
    let (feed, sender) = gas_feed();
    world.gas_feed_sender = Some(sender);
    let screen = GasTrackerScreen::new(chain, initial, feed);
    let stack = world.stack.as_mut().unwrap();
    stack.push(Box::new(screen));
}

#[given("the user opens the Gas Tracker with a refresh handle")]
async fn opens_gas_tracker_with_refresh(world: &mut AppWorld) {
    build_stack(world);
    let chain = *world.active_chain.get_or_insert(Chain::Ethereum);
    let initial = Some(world.gas_stub.expected(chain));
    let (feed, sender) = gas_feed();
    let (handle, listener) = gas_refresh_channel();
    world.gas_feed_sender = Some(sender);
    world.gas_refresh_listener = Some(listener);
    let screen = GasTrackerScreen::new(chain, initial, feed).with_refresh_handle(handle);
    let stack = world.stack.as_mut().unwrap();
    stack.push(Box::new(screen));
}

#[then(regex = r#"^the Gas Tracker shows slow (\d+) average (\d+) fast (\d+)$"#)]
async fn shows_tiers(world: &mut AppWorld, slow: u128, average: u128, fast: u128) {
    let stack = world.stack.as_ref().expect("stack");
    let g = current(stack).current().expect("primed");
    assert_eq!(g.slow.value(), slow);
    assert_eq!(g.average.value(), average);
    assert_eq!(g.fast.value(), fast);
}

// ---------------------------------------------------------------------------
// §11.2 — pause + manual refresh
// ---------------------------------------------------------------------------

#[when(regex = r#"^the user presses "p" on the Gas Tracker$"#)]
async fn presses_p(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    let screen = current_mut(stack);
    let _ = screen.handle_key(make_key(KeyCode::Char('p'), KeyModifiers::empty()));
}

#[when(regex = r#"^the gas feed emits a snapshot with base fee (\d+)$"#)]
async fn gas_feed_emits(world: &mut AppWorld, base: u128) {
    let chain = *world.active_chain.get_or_insert(Chain::Ethereum);
    let sender = world
        .gas_feed_sender
        .as_ref()
        .expect("gas feed sender captured by the Gas Tracker step");
    sender
        .updates_tx
        .send(snapshot_with_base(chain, base))
        .expect("feed open");

    let stack = world.stack.as_mut().expect("stack");
    let _ = current_mut(stack).tick();
}

#[then(regex = r#"^the Gas Tracker badge shows "([^"]+)"$"#)]
async fn badge_shows(world: &mut AppWorld, label: String) {
    let stack = world.stack.as_ref().expect("stack");
    let screen = current(stack);
    let paused = screen.is_paused();
    match label.as_str() {
        "paused" => assert!(paused, "expected paused badge"),
        "live" => assert!(!paused, "expected live badge"),
        other => panic!("unknown badge label {other:?}"),
    }
}

#[then("the Gas Tracker base fee is still the primed value")]
async fn base_fee_still_primed(world: &mut AppWorld) {
    let chain = *world.active_chain.get_or_insert(Chain::Ethereum);
    let expected = world.gas_stub.expected(chain).base_fee.value();
    let stack = world.stack.as_ref().expect("stack");
    let got = current(stack).current().expect("primed").base_fee.value();
    assert_eq!(got, expected, "paused screen must ignore feed updates");
}

#[when(regex = r#"^the user presses "Ctrl\+R" on the Gas Tracker$"#)]
async fn presses_ctrl_r(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    let screen = current_mut(stack);
    let _ = screen.handle_key(make_key(KeyCode::Char('r'), KeyModifiers::CONTROL));
}

#[then("a refresh kick has been queued on the handle")]
async fn refresh_kick_queued(world: &mut AppWorld) {
    let listener = world
        .gas_refresh_listener
        .as_mut()
        .expect("refresh listener stored when the scenario set up the handle");
    assert!(
        listener.try_recv().is_some(),
        "expected one kick on the refresh channel",
    );
}

// ---------------------------------------------------------------------------
// §11.3 — percentile histogram
// ---------------------------------------------------------------------------

#[when(regex = r#"^the gas feed emits base fees ([\d, ]+)$"#)]
async fn feed_emits_series(world: &mut AppWorld, list: String) {
    let chain = *world.active_chain.get_or_insert(Chain::Ethereum);
    let sender = world
        .gas_feed_sender
        .as_ref()
        .expect("gas feed sender captured by the Gas Tracker step");
    for raw in list.split(',') {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let base: u128 = trimmed.parse().expect("numeric");
        sender
            .updates_tx
            .send(snapshot_with_base(chain, base))
            .expect("feed open");
    }

    let stack = world.stack.as_mut().expect("stack");
    let _ = current_mut(stack).tick();
}

#[then(regex = r#"^the Gas Tracker histogram reads p25 (\d+) p50 (\d+) p75 (\d+)$"#)]
async fn histogram_reads(world: &mut AppWorld, p25: u128, p50: u128, p75: u128) {
    let stack = world.stack.as_ref().expect("stack");
    let p = current(stack).percentiles();
    assert_eq!(p.p25.value(), p25, "p25");
    assert_eq!(p.p50.value(), p50, "p50");
    assert_eq!(p.p75.value(), p75, "p75");
}

// ---------------------------------------------------------------------------
// §11.4 — unit converter modal
// ---------------------------------------------------------------------------

#[when(regex = r#"^the user presses "u" on the Gas Tracker$"#)]
async fn presses_u(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    let screen = current_mut(stack);
    let _ = screen.handle_key(make_key(KeyCode::Char('u'), KeyModifiers::empty()));
    assert!(screen.converter_open(), "u must open the modal");
}

#[when(regex = r#"^the user types "([^"]+)" into the unit converter$"#)]
async fn types_into_converter(world: &mut AppWorld, text: String) {
    let stack = world.stack.as_mut().expect("stack");
    let screen = current_mut(stack);
    assert!(
        screen.converter_open(),
        "the unit converter must be open before typing"
    );
    for ch in text.chars() {
        let _ = screen.handle_key(make_key(KeyCode::Char(ch), KeyModifiers::empty()));
    }
}

#[when("the user submits the unit converter")]
async fn submits_converter(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    let _ = current_mut(stack).handle_key(make_key(KeyCode::Enter, KeyModifiers::empty()));
}

#[then(regex = r#"^the Gas Tracker converter result is "([^"]+)"$"#)]
async fn converter_result_is(world: &mut AppWorld, expected: String) {
    let stack = world.stack.as_ref().expect("stack");
    let got = current(stack).converter_result().map(ToString::to_string);
    assert_eq!(got.as_deref(), Some(expected.as_str()));
}
