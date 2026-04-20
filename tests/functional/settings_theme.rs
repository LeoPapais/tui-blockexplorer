//! Theme presets on the Settings screen.
//!
//! See `plan/10-settings.md` section 12.7.

use std::sync::Arc;

use blockexplorer_tui::{
    adapters::ui::{
        AppConfigSnapshot, Command, CursorServices, PalettePreset, Screen, SettingsScreen,
    },
    application::ports::ClipboardPort,
    domain::Chain,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

use super::support::stubs::{StubClipboard, StubNavigationFactory};

fn snapshot() -> AppConfigSnapshot {
    AppConfigSnapshot {
        chain: Chain::Ethereum,
        alchemy_key_present: true,
        config_path_hint: None,
    }
}

fn render(screen: &SettingsScreen) -> Buffer {
    let backend = TestBackend::new(120, 30);
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
fn theme_section_lists_every_preset() {
    let screen = SettingsScreen::new(snapshot());

    let buffer = render(&screen);

    assert!(buffer_contains(&buffer, "Dark"));
    assert!(buffer_contains(&buffer, "Light"));
    assert!(buffer_contains(&buffer, "High contrast"));
    assert!(buffer_contains(&buffer, "Solarized"));
}

#[test]
fn pressing_the_preset_number_applies_it() {
    let mut screen = SettingsScreen::new(snapshot());
    assert_eq!(screen.palette(), PalettePreset::DarkDefault);

    let cmd = screen.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
    assert_eq!(cmd, Command::None);
    assert_eq!(screen.palette(), PalettePreset::HighContrast);
}

#[test]
fn the_active_preset_is_marked_with_a_star() {
    let screen = SettingsScreen::new(snapshot()).with_palette(PalettePreset::HighContrast);

    let buffer = render(&screen);

    let mut lines = Vec::new();
    let mut row = String::new();
    for y in 0..buffer.area.height {
        row.clear();
        for x in 0..buffer.area.width {
            row.push_str(buffer[(x, y)].symbol());
        }
        lines.push(row.clone());
    }

    let high_contrast_row = lines
        .iter()
        .find(|l| l.contains("High contrast"))
        .expect("high-contrast row must be rendered");
    assert!(
        high_contrast_row.contains('*'),
        "active preset must be marked with a star: {high_contrast_row:?}"
    );
}

#[test]
fn out_of_range_digit_is_ignored() {
    let mut screen = SettingsScreen::new(snapshot());
    screen.handle_key(KeyEvent::new(KeyCode::Char('9'), KeyModifiers::NONE));
    assert_eq!(screen.palette(), PalettePreset::DarkDefault);
}

#[test]
fn y_without_cursor_still_routes_to_clipboard_when_services_are_present() {
    // With no active cursor, `y` copies the first navigable field
    // (active_chain) so the user always gets something useful out of
    // the shortcut.
    let clipboard = StubClipboard::new();
    let services = CursorServices::new(
        Arc::new(clipboard.clone()) as Arc<dyn ClipboardPort>,
        Arc::new(StubNavigationFactory::new()),
        Chain::Ethereum,
    );
    let mut screen = SettingsScreen::new(snapshot()).with_cursor_services(services);
    assert!(!screen.cursor().is_active());

    screen.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

    assert_eq!(
        clipboard.last_copied().as_deref(),
        Some(Chain::Ethereum.display_name()),
    );
}
