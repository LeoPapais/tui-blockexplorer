//! Key-handling tests for [`GasTrackerScreen`].
//!
//! Covers §11.2 (pause + Ctrl+R), §11.3 (percentile histogram) and §11.4
//! (unit converter modal) of `plan/9-gas-tracker.md`.

use blockexplorer_tui::{
    adapters::ui::{Command, GasTrackerScreen, Screen, gas_feed, gas_tracker::gas_refresh_channel},
    domain::{Chain, GasSnapshot, Gwei, gas},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

fn snapshot(base: u128) -> GasSnapshot {
    GasSnapshot {
        chain: Chain::Ethereum,
        slow: Gwei::new(base + 1),
        average: Gwei::new(base + 2),
        fast: Gwei::new(base + 3),
        base_fee: Gwei::new(base),
        trend: vec![Gwei::new(base); 20],
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
}

#[test]
fn p_toggles_paused_flag() {
    let (feed, _sender) = gas_feed();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed);
    assert!(!screen.is_paused());

    assert_eq!(screen.handle_key(key(KeyCode::Char('p'))), Command::None);
    assert!(screen.is_paused());

    assert_eq!(screen.handle_key(key(KeyCode::Char('p'))), Command::None);
    assert!(!screen.is_paused());
}

#[test]
fn paused_screen_drops_incoming_snapshots_but_still_drains() {
    let (feed, sender) = gas_feed();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed);
    screen.handle_key(key(KeyCode::Char('p')));

    sender.updates_tx.send(snapshot(200)).expect("feed open");
    sender.updates_tx.send(snapshot(300)).expect("feed open");
    let _ = screen.tick();

    // Current snapshot stayed at the primed value.
    assert_eq!(screen.current().expect("primed").base_fee.value(), 10);
    // Receiver is still alive — the feed task can keep sending.
    assert!(sender.updates_tx.send(snapshot(400)).is_ok());
}

#[test]
fn live_screen_applies_incoming_snapshots() {
    let (feed, sender) = gas_feed();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed);
    sender.updates_tx.send(snapshot(77)).expect("feed open");
    let _ = screen.tick();
    assert_eq!(screen.current().expect("primed").base_fee.value(), 77);
}

#[test]
fn ctrl_r_returns_refresh_and_kicks_refresh_handle() {
    let (feed, _sender) = gas_feed();
    let (refresh_handle, mut refresh_listener) = gas_refresh_channel();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed)
        .with_refresh_handle(refresh_handle);

    assert_eq!(
        screen.handle_key(ctrl(KeyCode::Char('r'))),
        Command::Refresh
    );
    assert_eq!(
        screen.handle_key(ctrl(KeyCode::Char('R'))),
        Command::Refresh
    );
    assert!(refresh_listener.try_recv().is_some());
    assert!(refresh_listener.try_recv().is_some());
    assert!(refresh_listener.try_recv().is_none());
}

#[test]
fn ctrl_r_without_handle_still_returns_refresh_command() {
    let (feed, _sender) = gas_feed();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed);
    assert_eq!(
        screen.handle_key(ctrl(KeyCode::Char('r'))),
        Command::Refresh
    );
}

#[test]
fn ctrl_r_unpauses_and_next_snapshot_is_applied() {
    let (feed, sender) = gas_feed();
    let (refresh_handle, _listener) = gas_refresh_channel();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed)
        .with_refresh_handle(refresh_handle);
    screen.handle_key(key(KeyCode::Char('p')));
    assert!(screen.is_paused());

    screen.handle_key(ctrl(KeyCode::Char('r')));
    assert!(!screen.is_paused(), "Ctrl+R must unpause the screen");

    sender.updates_tx.send(snapshot(123)).expect("feed open");
    let _ = screen.tick();
    assert_eq!(screen.current().expect("primed").base_fee.value(), 123);
}

#[test]
fn rolling_window_exposes_percentiles_over_applied_samples() {
    let (feed, sender) = gas_feed();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, None, feed);
    for base in [5u128, 15, 25, 35, 45] {
        sender.updates_tx.send(snapshot(base)).expect("feed open");
    }
    let _ = screen.tick();

    let p = screen.percentiles();
    // Sorted base fees: 5, 15, 25, 35, 45 (n=5).
    // Nearest-rank: ceil(1.25)=2 -> 15; ceil(2.5)=3 -> 25; ceil(3.75)=4 -> 35.
    assert_eq!(p.p25, Gwei::new(15));
    assert_eq!(p.p50, Gwei::new(25));
    assert_eq!(p.p75, Gwei::new(35));
}

#[test]
fn empty_history_returns_zeroed_percentiles() {
    let (feed, _sender) = gas_feed();
    let screen = GasTrackerScreen::new(Chain::Ethereum, None, feed);
    assert_eq!(screen.percentiles(), gas::Percentiles::empty());
}

#[test]
fn u_opens_unit_converter_modal() {
    let (feed, _sender) = gas_feed();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed);
    assert!(!screen.converter_open());
    screen.handle_key(key(KeyCode::Char('u')));
    assert!(screen.converter_open());
}

#[test]
fn esc_closes_the_modal_without_popping_the_screen() {
    let (feed, _sender) = gas_feed();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed);
    screen.handle_key(key(KeyCode::Char('u')));
    assert!(screen.converter_open());

    assert_eq!(screen.handle_key(key(KeyCode::Esc)), Command::None);
    assert!(!screen.converter_open());
}

#[test]
fn esc_without_modal_still_pops() {
    let (feed, _sender) = gas_feed();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed);
    assert_eq!(screen.handle_key(key(KeyCode::Esc)), Command::Pop);
}

#[test]
fn converter_converts_one_ether_to_gwei_on_enter() {
    let (feed, _sender) = gas_feed();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed);
    screen.handle_key(key(KeyCode::Char('u')));
    // Default from -> to is Ether -> Gwei.
    assert_eq!(
        screen.converter_units(),
        (gas::Unit::Ether, gas::Unit::Gwei)
    );

    screen.handle_key(key(KeyCode::Char('1')));
    assert_eq!(screen.converter_input(), "1");

    assert_eq!(screen.handle_key(key(KeyCode::Enter)), Command::None);
    assert_eq!(screen.converter_result(), Some("1000000000"));
    assert_eq!(screen.converter_error(), None);
}

#[test]
fn converter_reports_validation_error_for_negative_input() {
    let (feed, _sender) = gas_feed();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed);
    screen.handle_key(key(KeyCode::Char('u')));
    screen.handle_key(key(KeyCode::Char('-')));
    screen.handle_key(key(KeyCode::Char('1')));
    screen.handle_key(key(KeyCode::Enter));

    assert!(
        screen
            .converter_error()
            .is_some_and(|e| e.contains("negative")),
        "expected a negative-value error, got {:?}",
        screen.converter_error(),
    );
    assert_eq!(screen.converter_result(), None);
}

#[test]
fn converter_left_right_cycle_source_unit() {
    let (feed, _sender) = gas_feed();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed);
    screen.handle_key(key(KeyCode::Char('u')));
    assert_eq!(
        screen.converter_units(),
        (gas::Unit::Ether, gas::Unit::Gwei)
    );

    screen.handle_key(key(KeyCode::Right));
    assert_eq!(screen.converter_units().0, gas::Unit::Wei);
    screen.handle_key(key(KeyCode::Right));
    assert_eq!(screen.converter_units().0, gas::Unit::Gwei);
    screen.handle_key(key(KeyCode::Right));
    assert_eq!(screen.converter_units().0, gas::Unit::Ether);

    screen.handle_key(key(KeyCode::Left));
    assert_eq!(screen.converter_units().0, gas::Unit::Gwei);
}

#[test]
fn converter_up_down_cycle_target_unit() {
    let (feed, _sender) = gas_feed();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed);
    screen.handle_key(key(KeyCode::Char('u')));
    assert_eq!(screen.converter_units().1, gas::Unit::Gwei);

    screen.handle_key(key(KeyCode::Down));
    assert_eq!(screen.converter_units().1, gas::Unit::Ether);
    screen.handle_key(key(KeyCode::Up));
    assert_eq!(screen.converter_units().1, gas::Unit::Gwei);
}

#[test]
fn converter_backspace_edits_input() {
    let (feed, _sender) = gas_feed();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed);
    screen.handle_key(key(KeyCode::Char('u')));
    screen.handle_key(key(KeyCode::Char('1')));
    screen.handle_key(key(KeyCode::Char('2')));
    screen.handle_key(key(KeyCode::Backspace));
    assert_eq!(screen.converter_input(), "1");
}

#[test]
fn converter_ignores_non_numeric_characters() {
    let (feed, _sender) = gas_feed();
    let mut screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot(10)), feed);
    screen.handle_key(key(KeyCode::Char('u')));
    // 'p' is neither digit nor '.' nor '-' and must be ignored.
    screen.handle_key(key(KeyCode::Char('p')));
    assert_eq!(screen.converter_input(), "");
    // Pause toggle must NOT trigger while modal is open.
    assert!(!screen.is_paused());
}
