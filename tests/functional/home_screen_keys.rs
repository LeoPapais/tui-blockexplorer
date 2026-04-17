//! Key-handling tests for [`HomeScreen`].
//!
//! See `plan/12-screen-runtime.md` section 2.4.

use blockexplorer_tui::adapters::ui::{Command, HomeScreen, Screen};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn q_returns_quit() {
    let mut screen = HomeScreen::with_demo_data();
    assert_eq!(screen.handle_key(key(KeyCode::Char('q'))), Command::Quit);
}

#[test]
fn escape_returns_pop() {
    let mut screen = HomeScreen::with_demo_data();
    assert_eq!(screen.handle_key(key(KeyCode::Esc)), Command::Pop);
}

#[test]
fn other_keys_are_inert_for_now() {
    let mut screen = HomeScreen::with_demo_data();
    assert_eq!(screen.handle_key(key(KeyCode::Char('c'))), Command::None);
    assert_eq!(screen.handle_key(key(KeyCode::Enter)), Command::None);
    assert_eq!(screen.handle_key(key(KeyCode::Tab)), Command::None);
}

#[test]
fn title_is_home() {
    let screen = HomeScreen::with_demo_data();
    assert_eq!(screen.title(), "Home");
}
