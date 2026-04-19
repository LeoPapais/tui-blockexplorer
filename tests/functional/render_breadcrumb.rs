//! Unit tests for `adapters::ui::breadcrumb::render_breadcrumb`.
//!
//! See `plan/0-general-architecture.md` §3 "Global screen layout" and
//! `plan/15-backlog.md` §8.16 (application shell bullet).
//!
//! The helper is pure — no I/O, no async — so a plain `#[test]`
//! drives it. We use a deliberately dumb `StubScreen` to avoid
//! pulling in real UI feeds for a helper that only cares about the
//! `title()` strings.

use blockexplorer_tui::adapters::ui::{
    BREADCRUMB_SEPARATOR, Command, Screen, ScreenStack, breadcrumb_segments, render_breadcrumb,
};
use crossterm::event::KeyEvent;
use pretty_assertions::assert_eq;
use ratatui::{Frame, layout::Rect};

struct StubScreen {
    title: String,
}

impl StubScreen {
    fn boxed(title: &str) -> Box<dyn Screen> {
        Box::new(Self {
            title: title.to_owned(),
        })
    }
}

impl Screen for StubScreen {
    fn title(&self) -> &str {
        &self.title
    }
    fn render(&self, _: &mut Frame<'_>, _: Rect) {}
    fn handle_key(&mut self, _: KeyEvent) -> Command {
        Command::None
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[test]
fn empty_stack_renders_an_empty_string() {
    let stack = ScreenStack::new();
    assert_eq!(render_breadcrumb(&stack), "");
    assert!(breadcrumb_segments(&stack).is_empty());
}

#[test]
fn single_screen_renders_just_its_title() {
    let mut stack = ScreenStack::new();
    stack.push(StubScreen::boxed("Home"));
    assert_eq!(render_breadcrumb(&stack), "Home");
    assert_eq!(breadcrumb_segments(&stack), vec!["Home".to_owned()]);
}

#[test]
fn stack_renders_bottom_up_with_the_standard_separator() {
    let mut stack = ScreenStack::new();
    stack.push(StubScreen::boxed("Home"));
    stack.push(StubScreen::boxed("Block 21345678"));
    stack.push(StubScreen::boxed("Tx 0xabc\u{2026}"));
    assert_eq!(
        render_breadcrumb(&stack),
        format!(
            "Home{s}Block 21345678{s}Tx 0xabc\u{2026}",
            s = BREADCRUMB_SEPARATOR
        )
    );
    assert_eq!(
        breadcrumb_segments(&stack),
        vec![
            "Home".to_owned(),
            "Block 21345678".to_owned(),
            "Tx 0xabc\u{2026}".to_owned(),
        ],
    );
}

#[test]
fn open_modal_does_not_affect_the_breadcrumb() {
    let mut stack = ScreenStack::new();
    stack.push(StubScreen::boxed("Home"));
    stack.push(StubScreen::boxed("Settings"));
    // A modal is rendered on top of the stack but does not belong to
    // the navigation trail — the user returns to "Settings" when they
    // dismiss it.
    stack.open_modal(StubScreen::boxed("Keybind Conflict"));
    assert_eq!(
        render_breadcrumb(&stack),
        format!("Home{s}Settings", s = BREADCRUMB_SEPARATOR)
    );
}
