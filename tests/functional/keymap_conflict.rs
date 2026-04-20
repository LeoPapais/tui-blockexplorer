//! KeyMap + conflict detection.
//!
//! See `plan/10-settings.md` section 12.4.

use blockexplorer_tui::{
    adapters::ui::{KeyBindConflictModal, Screen},
    domain::{Action, KeyBinding, KeyMap, ScreenId, key_binding::format_key},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

fn binding(screen: ScreenId, key: char, action: Action) -> KeyBinding {
    KeyBinding {
        screen,
        key: KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE),
        action,
    }
}

#[test]
fn builtin_map_has_no_conflicts() {
    let map = KeyMap::builtin();
    assert!(!map.is_empty(), "builtin keymap must include bindings");
    // Sanity: the classic Home shortcuts resolve.
    let enter_search = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    assert_eq!(
        map.resolve(ScreenId::Home, enter_search),
        Some(Action::OpenSearch)
    );
}

#[test]
fn duplicate_same_action_is_not_a_conflict() {
    let entries = vec![
        binding(ScreenId::Home, 'q', Action::Quit),
        binding(ScreenId::Home, 'q', Action::Quit),
    ];

    let map = KeyMap::from_entries(&entries).expect("dup same action OK");

    let q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    assert_eq!(map.resolve(ScreenId::Home, q), Some(Action::Quit));
}

#[test]
fn same_key_mapped_to_two_actions_surfaces_conflict() {
    let entries = vec![
        binding(ScreenId::Home, 'q', Action::Quit),
        binding(ScreenId::Home, 'q', Action::Back),
    ];

    let err = KeyMap::from_entries(&entries).expect_err("must conflict");

    assert_eq!(err.conflicts.len(), 1);
    let conflict = &err.conflicts[0];
    assert_eq!(conflict.screen, ScreenId::Home);
    assert!(conflict.actions.contains(&Action::Quit));
    assert!(conflict.actions.contains(&Action::Back));
}

#[test]
fn conflict_report_display_lists_every_offender() {
    let entries = vec![
        binding(ScreenId::Home, 'q', Action::Quit),
        binding(ScreenId::Home, 'q', Action::Back),
        binding(ScreenId::Search, 'x', Action::OpenSearch),
        binding(ScreenId::Search, 'x', Action::OpenSettings),
    ];

    let err = KeyMap::from_entries(&entries).expect_err("must conflict");

    assert_eq!(err.conflicts.len(), 2);
    let rendered = format!("{err}");
    assert!(rendered.contains("2 keybind conflict(s)"));
    assert!(rendered.contains("Home"));
    assert!(rendered.contains("Search"));
}

#[test]
fn global_binding_resolves_when_screen_has_no_match() {
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let entries = vec![KeyBinding {
        screen: ScreenId::Global,
        key: enter,
        action: Action::OpenSettings,
    }];

    let map = KeyMap::from_entries(&entries).expect("ok");

    assert_eq!(
        map.resolve(ScreenId::Home, enter),
        Some(Action::OpenSettings)
    );
    assert_eq!(
        map.resolve(ScreenId::BlockDetail, enter),
        Some(Action::OpenSettings)
    );
}

#[test]
fn screen_specific_binding_shadows_global() {
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let entries = vec![
        KeyBinding {
            screen: ScreenId::Global,
            key: enter,
            action: Action::OpenSettings,
        },
        KeyBinding {
            screen: ScreenId::Home,
            key: enter,
            action: Action::OpenSearch,
        },
    ];

    let map = KeyMap::from_entries(&entries).expect("ok");

    assert_eq!(map.resolve(ScreenId::Home, enter), Some(Action::OpenSearch));
    assert_eq!(
        map.resolve(ScreenId::TxDetail, enter),
        Some(Action::OpenSettings)
    );
}

#[test]
fn format_key_covers_modifiers_and_special_keys() {
    let ctrl_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
    assert_eq!(format_key(ctrl_r), "Ctrl+r");

    let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(format_key(esc), "Esc");

    let shift_tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT);
    assert_eq!(format_key(shift_tab), "Shift+Tab");
}

fn render_modal(modal: &KeyBindConflictModal) -> Buffer {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| modal.render(frame, frame.area()))
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
fn conflict_modal_renders_offending_screen_key_and_actions() {
    let entries = vec![
        binding(ScreenId::Home, 'q', Action::Quit),
        binding(ScreenId::Home, 'q', Action::Back),
    ];
    let err = KeyMap::from_entries(&entries).expect_err("must conflict");
    let modal = KeyBindConflictModal::new(err);

    let buffer = render_modal(&modal);

    assert!(buffer_contains(&buffer, "Home"));
    assert!(buffer_contains(&buffer, "Quit"));
    assert!(buffer_contains(&buffer, "Back"));
    assert!(buffer_contains(&buffer, "Esc"));
}

#[test]
fn conflict_modal_pops_on_esc() {
    use blockexplorer_tui::adapters::ui::Command;
    let entries = vec![
        binding(ScreenId::Home, 'q', Action::Quit),
        binding(ScreenId::Home, 'q', Action::Back),
    ];
    let err = KeyMap::from_entries(&entries).expect_err("must conflict");
    let mut modal = KeyBindConflictModal::new(err);

    let cmd = modal.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(cmd, Command::Pop);
}
