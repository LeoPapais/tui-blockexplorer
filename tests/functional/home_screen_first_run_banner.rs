//! First-run banner on the Home screen.
//!
//! See `plan/10-settings.md` section 12.2. When the binary boots
//! without an Alchemy key, Home renders a dismissible banner pointing
//! the user at Settings → Credentials. Esc / Enter dismiss it; `s`
//! still opens Settings.

use blockexplorer_tui::{
    adapters::ui::{Command, HomeScreen, Screen},
    application::{ConnectionStatus, HomeViewModel},
    domain::Chain,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

fn empty_view() -> HomeViewModel {
    HomeViewModel {
        chain: Chain::Ethereum,
        network: None,
        gas: None,
        connection: ConnectionStatus::Connected,
    }
}

fn render(screen: &HomeScreen) -> Buffer {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| screen.render(frame, frame.area()))
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

#[test]
fn banner_renders_when_first_run_hint_is_set() {
    let screen = HomeScreen::new(empty_view()).with_first_run_hint(true);

    let buffer = render(&screen);

    assert!(
        buffer_contains(&buffer, "Set up credentials"),
        "banner must mention the setup hint"
    );
    assert!(
        buffer_contains(&buffer, "s"),
        "banner must mention the `s` shortcut"
    );
}

#[test]
fn banner_is_absent_when_first_run_hint_is_false() {
    let screen = HomeScreen::new(empty_view());

    let buffer = render(&screen);

    assert!(
        !buffer_contains(&buffer, "Set up credentials"),
        "banner must not appear when the hint is off"
    );
}

#[test]
fn esc_dismisses_the_banner_without_popping_the_screen() {
    let mut screen = HomeScreen::new(empty_view()).with_first_run_hint(true);

    let command = screen.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert_eq!(command, Command::None);

    let buffer = render(&screen);
    assert!(
        !buffer_contains(&buffer, "Set up credentials"),
        "banner must disappear after Esc"
    );
}

#[test]
fn enter_dismisses_the_banner_without_popping_the_screen() {
    let mut screen = HomeScreen::new(empty_view()).with_first_run_hint(true);

    let command = screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(command, Command::None);

    let buffer = render(&screen);
    assert!(!buffer_contains(&buffer, "Set up credentials"));
}

#[test]
fn esc_pops_the_screen_after_the_banner_has_been_dismissed() {
    let mut screen = HomeScreen::new(empty_view()).with_first_run_hint(true);

    screen.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let command = screen.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert_eq!(command, Command::Pop);
}
