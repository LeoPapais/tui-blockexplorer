//! Functional tests for the `HelpModal`.
//!
//! See `plan/12-screen-runtime.md` §7.

use blockexplorer_tui::adapters::ui::{Command, HelpModal, Screen};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn press(modal: &mut HelpModal, code: KeyCode) -> Command {
    modal.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
}

#[test]
fn help_modal_renders_the_entries_passed_in() {
    let modal = HelpModal::new(
        "Home",
        vec![
            ("q".to_string(), "Quit".to_string()),
            ("?".to_string(), "Show help".to_string()),
        ],
    );

    let body = modal.body();
    assert!(body.contains("q"));
    assert!(body.contains("Quit"));
    assert!(body.contains("?"));
    assert!(body.contains("Show help"));
}

#[test]
fn esc_closes_the_modal() {
    let mut modal = HelpModal::new("Home", Vec::new());
    assert_eq!(press(&mut modal, KeyCode::Esc), Command::CloseModal);
}

#[test]
fn enter_closes_the_modal() {
    let mut modal = HelpModal::new("Home", Vec::new());
    assert_eq!(press(&mut modal, KeyCode::Enter), Command::CloseModal);
}

#[test]
fn question_mark_closes_the_modal_too() {
    let mut modal = HelpModal::new("Home", Vec::new());
    assert_eq!(press(&mut modal, KeyCode::Char('?')), Command::CloseModal);
}

#[test]
fn other_keys_are_swallowed() {
    let mut modal = HelpModal::new("Home", Vec::new());
    assert_eq!(press(&mut modal, KeyCode::Char('j')), Command::None);
}
