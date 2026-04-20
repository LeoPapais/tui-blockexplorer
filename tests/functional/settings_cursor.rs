//! Field-cursor flow on the Settings screen.
//!
//! See `plan/17-navigable-values.md` §6.

use std::sync::Arc;

use blockexplorer_tui::{
    adapters::ui::{AppConfigSnapshot, Command, CursorServices, Screen, SettingsScreen},
    domain::Chain,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use crate::support::stubs::{StubClipboard, StubNavigationFactory};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn settings_cursor_y_copies_config_path() {
    let clipboard = Arc::new(StubClipboard::new());
    let nav = Arc::new(StubNavigationFactory::new());
    let services = CursorServices::new(clipboard.clone(), nav.clone(), Chain::Ethereum);
    let mut screen = SettingsScreen::new(AppConfigSnapshot {
        chain: Chain::Ethereum,
        alchemy_key_present: true,
        config_path_hint: Some("/home/alice/.config/blockexplorer-tui/config.toml".to_string()),
    })
    .with_cursor_services(services);

    // Field order on Settings is [active_chain, config_path]. Step
    // once past active_chain.
    screen.handle_key(key(KeyCode::Right));
    screen.handle_key(key(KeyCode::Right));

    screen.handle_key(key(KeyCode::Char('y')));

    assert_eq!(
        clipboard.last_copied(),
        Some("/home/alice/.config/blockexplorer-tui/config.toml".to_string()),
    );

    // Plain field → Enter must not push a screen.
    let _ = screen.handle_key(key(KeyCode::Enter));
    assert!(nav.recorded_values().is_empty());
}

#[test]
fn esc_pops_screen_even_when_cursor_is_active() {
    let clipboard = Arc::new(StubClipboard::new());
    let nav = Arc::new(StubNavigationFactory::new());
    let services = CursorServices::new(clipboard, nav, Chain::Ethereum);
    let mut screen = SettingsScreen::new(AppConfigSnapshot {
        chain: Chain::Ethereum,
        alchemy_key_present: true,
        config_path_hint: Some("/tmp/config.toml".to_string()),
    })
    .with_cursor_services(services);

    screen.handle_key(key(KeyCode::Right));
    assert!(screen.cursor().is_active());
    assert_eq!(screen.handle_key(key(KeyCode::Esc)), Command::Pop);
}

#[test]
fn backspace_deactivates_cursor_without_popping() {
    let clipboard = Arc::new(StubClipboard::new());
    let nav = Arc::new(StubNavigationFactory::new());
    let services = CursorServices::new(clipboard, nav, Chain::Ethereum);
    let mut screen = SettingsScreen::new(AppConfigSnapshot {
        chain: Chain::Ethereum,
        alchemy_key_present: true,
        config_path_hint: Some("/tmp/config.toml".to_string()),
    })
    .with_cursor_services(services);

    screen.handle_key(key(KeyCode::Right));
    assert!(screen.cursor().is_active());
    assert_eq!(screen.handle_key(key(KeyCode::Backspace)), Command::None);
    assert!(!screen.cursor().is_active());
}
