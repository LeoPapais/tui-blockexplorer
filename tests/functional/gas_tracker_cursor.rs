//! Cursor tests for the Gas Tracker screen.
//!
//! Every navigable value on this screen is `Plain` (gwei tiers). We
//! only assert on the clipboard side effect. See
//! `plan/17-navigable-values.md` §6 + §8.1.

use std::sync::Arc;

use blockexplorer_tui::{
    adapters::ui::{Command, CursorServices, GasTrackerScreen, Screen, gas_feed},
    application::ports::ClipboardPort,
    domain::{Chain, GasSnapshot, Gwei},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::support::stubs::{StubClipboard, StubNavigationFactory};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn snapshot() -> GasSnapshot {
    GasSnapshot {
        chain: Chain::Ethereum,
        slow: Gwei::new(12),
        average: Gwei::new(14),
        fast: Gwei::new(18),
        base_fee: Gwei::new(11),
        trend: vec![Gwei::new(11), Gwei::new(12)],
    }
}

fn wire_services() -> (GasTrackerScreen, StubClipboard, StubNavigationFactory) {
    let (feed, sender) = gas_feed();
    let clipboard = StubClipboard::new();
    let nav = StubNavigationFactory::new();
    let services = CursorServices::new(
        Arc::new(clipboard.clone()) as Arc<dyn ClipboardPort>,
        Arc::new(nav.clone()),
        Chain::Ethereum,
    );
    let screen = GasTrackerScreen::new(Chain::Ethereum, Some(snapshot()), feed)
        .with_cursor_services(services);
    std::mem::drop(sender);
    (screen, clipboard, nav)
}

#[test]
fn cursor_y_copies_gwei_value_without_navigation() {
    let (mut screen, clipboard, nav) = wire_services();
    screen.handle_key(key(KeyCode::Right));
    let cmd = screen.handle_key(key(KeyCode::Char('y')));
    assert_eq!(cmd, Command::None);
    assert_eq!(clipboard.last_copied().as_deref(), Some("12 gwei"));
    assert!(
        nav.recorded().is_empty(),
        "plain values must never navigate",
    );
}

#[test]
fn cursor_enter_on_gwei_value_is_a_noop() {
    let (mut screen, _, nav) = wire_services();
    screen.handle_key(key(KeyCode::Right));
    let cmd = screen.handle_key(key(KeyCode::Enter));
    assert_eq!(cmd, Command::None);
    assert!(nav.recorded().is_empty());
}

#[test]
fn backspace_deactivates_cursor_without_popping() {
    let (mut screen, _, _) = wire_services();
    screen.handle_key(key(KeyCode::Right));
    assert!(screen.cursor().is_active());
    assert_eq!(screen.handle_key(key(KeyCode::Backspace)), Command::None);
    assert!(!screen.cursor().is_active());
}

#[test]
fn esc_pops_screen_even_when_cursor_is_active() {
    let (mut screen, _, _) = wire_services();
    screen.handle_key(key(KeyCode::Right));
    assert!(screen.cursor().is_active());
    assert_eq!(screen.handle_key(key(KeyCode::Esc)), Command::Pop);
}
