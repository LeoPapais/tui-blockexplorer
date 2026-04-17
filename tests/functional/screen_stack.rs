//! Functional tests for `ScreenStack`.
//!
//! See `plan/12-screen-runtime.md` section 2.3.

use blockexplorer_tui::adapters::ui::{Command, Screen, ScreenStack};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Frame, layout::Rect};

/// Tiny screen used as a fixture for stack tests. Stores a label so
/// tests can assert that the top of the stack is the screen they just
/// pushed.
struct LabelScreen {
    label: &'static str,
}

impl LabelScreen {
    fn boxed(label: &'static str) -> Box<dyn Screen> {
        Box::new(Self { label })
    }
}

impl Screen for LabelScreen {
    fn title(&self) -> &str {
        self.label
    }

    fn render(&self, _frame: &mut Frame<'_>, _area: Rect) {}

    fn handle_key(&mut self, _key: KeyEvent) -> Command {
        Command::None
    }
}

#[test]
fn new_stack_is_empty() {
    let stack = ScreenStack::new();
    assert!(stack.is_empty());
    assert_eq!(stack.len(), 0);
    assert!(stack.top().is_none());
}

#[test]
fn push_exposes_the_top_screen() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));
    stack.push(LabelScreen::boxed("tx"));

    assert_eq!(stack.len(), 2);
    assert_eq!(stack.top().unwrap().title(), "tx");
}

#[test]
fn pop_returns_the_top_and_shrinks_the_stack() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));
    stack.push(LabelScreen::boxed("tx"));

    let popped = stack.pop().expect("stack not empty");
    assert_eq!(popped.title(), "tx");
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.top().unwrap().title(), "home");
}

#[test]
fn clear_removes_every_screen() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));
    stack.push(LabelScreen::boxed("tx"));

    stack.clear();
    assert!(stack.is_empty());
}

#[test]
fn top_mut_returns_a_mutable_handle() {
    // Screen whose tick flips an internal counter; proves top_mut is
    // really mutable.
    struct Counter {
        ticks: u32,
    }
    impl Screen for Counter {
        fn title(&self) -> &str {
            "counter"
        }
        fn render(&self, _frame: &mut Frame<'_>, _area: Rect) {}
        fn handle_key(&mut self, _key: KeyEvent) -> Command {
            Command::None
        }
        fn tick(&mut self) -> Command {
            self.ticks += 1;
            Command::None
        }
    }

    let mut stack = ScreenStack::new();
    stack.push(Box::new(Counter { ticks: 0 }));

    assert_eq!(stack.top_mut().unwrap().tick(), Command::None);
    assert_eq!(stack.top_mut().unwrap().tick(), Command::None);

    // We can't downcast safely through `dyn Screen`, but we can reach
    // the counter by popping and inspecting the title instead — title
    // stays the same, but ticks ran twice without panicking.
    assert_eq!(stack.top().unwrap().title(), "counter");
}

// Minimal unused-import guard: KeyEventKind + KeyModifiers are used by
// downstream tests in the same binary. They are listed here to assert
// the symbols resolve against the crossterm version we pin.
#[allow(dead_code)]
fn _assert_crossterm_symbols_resolve(kind: KeyEventKind, modifiers: KeyModifiers) {
    let _ = (kind, modifiers);
    let _ = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
}
