//! Cursor-driven key tests for [`HomeScreen`].
//!
//! See `plan/17-navigable-values.md` §6 (Home row) and §8.1.

use std::sync::Arc;

use blockexplorer_tui::{
    adapters::ui::{Command, CursorServices, HomeScreen, Screen},
    application::ports::ClipboardPort,
    domain::{BlockNumber, Chain, NavigableValue},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::support::stubs::{StubClipboard, StubNavigationFactory};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn wire_services() -> (HomeScreen, StubClipboard, StubNavigationFactory) {
    let clipboard = StubClipboard::new();
    let nav = StubNavigationFactory::new();
    let services = CursorServices::new(
        Arc::new(clipboard.clone()) as Arc<dyn ClipboardPort>,
        Arc::new(nav.clone()),
        Chain::Ethereum,
    );
    let screen = HomeScreen::with_demo_data().with_cursor_services(services);
    (screen, clipboard, nav)
}

#[test]
fn cursor_starts_inactive_on_home() {
    let (screen, _, _) = wire_services();
    assert!(!screen.cursor().is_active());
}

#[test]
fn first_arrow_press_activates_the_cursor_on_block_number() {
    let (mut screen, _, _) = wire_services();
    assert_eq!(screen.handle_key(key(KeyCode::Right)), Command::None);
    assert!(screen.cursor().is_active());
    let fields = screen.navigable_fields();
    let current = screen.cursor().current(&fields).expect("cursor");
    assert_eq!(current.label, "latest_block");
    assert_eq!(
        current.value,
        NavigableValue::BlockNumber(BlockNumber::new(21_345_678))
    );
}

#[test]
fn y_copies_block_number_through_the_clipboard_port() {
    let (mut screen, clipboard, _) = wire_services();
    screen.handle_key(key(KeyCode::Right));
    let cmd = screen.handle_key(key(KeyCode::Char('y')));
    assert_eq!(cmd, Command::None);
    assert_eq!(clipboard.last_copied().as_deref(), Some("21345678"));
}

#[test]
fn enter_on_block_number_asks_the_factory_for_a_push() {
    let (mut screen, _, nav) = wire_services();
    screen.handle_key(key(KeyCode::Right));
    screen.handle_key(key(KeyCode::Enter));
    let recorded = nav.recorded();
    assert_eq!(recorded.len(), 1);
    assert_eq!(
        recorded[0].0,
        NavigableValue::BlockNumber(BlockNumber::new(21_345_678))
    );
    assert_eq!(recorded[0].1, Chain::Ethereum);
}

#[test]
fn backspace_deactivates_the_cursor_without_popping() {
    let (mut screen, _, _) = wire_services();
    screen.handle_key(key(KeyCode::Right));
    assert!(screen.cursor().is_active());
    let cmd = screen.handle_key(key(KeyCode::Backspace));
    assert_eq!(cmd, Command::None);
    assert!(!screen.cursor().is_active());
}

#[test]
fn esc_while_cursor_inactive_still_pops_the_screen() {
    let (mut screen, _, _) = wire_services();
    let cmd = screen.handle_key(key(KeyCode::Esc));
    assert_eq!(cmd, Command::Pop);
}

#[test]
fn esc_pops_screen_even_when_cursor_is_active() {
    let (mut screen, _, _) = wire_services();
    screen.handle_key(key(KeyCode::Right));
    assert!(screen.cursor().is_active());
    let cmd = screen.handle_key(key(KeyCode::Esc));
    assert_eq!(cmd, Command::Pop);
}
