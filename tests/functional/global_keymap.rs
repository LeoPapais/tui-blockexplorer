//! Functional tests for the dispatcher-level `GlobalKeyMap`.
//!
//! The global map intercepts keys *before* the top screen's
//! `handle_key`, so behaviours like "open Search from any screen" do
//! not depend on each screen being taught the `/` binding.
//!
//! See `plan/12-screen-runtime.md` §7.

use blockexplorer_tui::adapters::ui::{Command, GlobalKeyMap, Screen};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Frame, layout::Rect};

struct Marker {
    label: &'static str,
}

impl Screen for Marker {
    fn title(&self) -> &str {
        self.label
    }
    fn render(&self, _frame: &mut Frame<'_>, _area: Rect) {}
    fn handle_key(&mut self, _key: KeyEvent) -> Command {
        Command::None
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn empty_map_returns_none_for_every_key() {
    let map = GlobalKeyMap::default();
    assert!(matches!(map.dispatch(key(KeyCode::Char('/'))), Command::None));
    assert!(matches!(map.dispatch(key(KeyCode::Char('?'))), Command::None));
    assert!(matches!(map.dispatch(key(KeyCode::Char('q'))), Command::None));
}

#[test]
fn slash_opens_search_as_a_modal_when_configured() {
    let map = GlobalKeyMap::default()
        .with_search_factory(|| Box::new(Marker { label: "Search" }));
    let cmd = map.dispatch(key(KeyCode::Char('/')));
    match cmd {
        Command::OpenModal(screen) => assert_eq!(screen.title(), "Search"),
        other => panic!("expected OpenModal(Search), got {other:?}"),
    }
}

#[test]
fn question_mark_opens_help_as_a_modal_when_configured() {
    let map = GlobalKeyMap::default()
        .with_help_factory(|| Box::new(Marker { label: "Help" }));
    let cmd = map.dispatch(key(KeyCode::Char('?')));
    match cmd {
        Command::OpenModal(screen) => assert_eq!(screen.title(), "Help"),
        other => panic!("expected OpenModal(Help), got {other:?}"),
    }
}

#[test]
fn unrelated_keys_are_not_intercepted() {
    let map = GlobalKeyMap::default()
        .with_search_factory(|| Box::new(Marker { label: "Search" }))
        .with_help_factory(|| Box::new(Marker { label: "Help" }));
    assert!(matches!(map.dispatch(key(KeyCode::Char('q'))), Command::None));
    assert!(matches!(map.dispatch(key(KeyCode::Enter)), Command::None));
}

#[test]
fn modifiers_prevent_interception() {
    // `Ctrl+/` should not trigger global search — only the plain key.
    let map = GlobalKeyMap::default()
        .with_search_factory(|| Box::new(Marker { label: "Search" }));
    let ctrl_slash = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::CONTROL);
    assert!(matches!(map.dispatch(ctrl_slash), Command::None));
}
