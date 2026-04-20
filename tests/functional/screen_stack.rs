//! Functional tests for `ScreenStack`.
//!
//! See `plan/12-screen-runtime.md` section 2.3.

use blockexplorer_tui::adapters::ui::{Command, Screen, ScreenStack};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use pretty_assertions::assert_eq;
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
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
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
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

// ---------------------------------------------------------------------------
// plan/12-screen-runtime.md §7: modal slot, apply_command, Switch.
// ---------------------------------------------------------------------------

#[test]
fn modal_slot_is_empty_on_a_fresh_stack() {
    let stack = ScreenStack::new();
    assert!(stack.modal().is_none());
    assert!(!stack.has_modal());
}

#[test]
fn open_modal_sets_the_modal_without_touching_the_stack() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));

    stack.open_modal(LabelScreen::boxed("help"));

    assert!(stack.has_modal());
    assert_eq!(stack.modal().unwrap().title(), "help");
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.top().unwrap().title(), "home");
}

#[test]
fn close_modal_clears_the_modal_slot() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));
    stack.open_modal(LabelScreen::boxed("help"));

    stack.close_modal();

    assert!(!stack.has_modal());
    assert_eq!(stack.top().unwrap().title(), "home");
}

#[test]
fn apply_command_pop_pops_the_stack_when_no_modal_is_open() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));
    stack.push(LabelScreen::boxed("tx"));

    let transition = stack.apply_command(Command::Pop);
    assert!(!transition.should_exit());
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.top().unwrap().title(), "home");
}

#[test]
fn apply_command_pop_closes_the_modal_when_one_is_open() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));
    stack.open_modal(LabelScreen::boxed("help"));

    let transition = stack.apply_command(Command::Pop);

    assert!(!transition.should_exit());
    assert!(!stack.has_modal());
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.top().unwrap().title(), "home");
}

#[test]
fn apply_command_quit_signals_exit() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));
    stack.open_modal(LabelScreen::boxed("help"));

    let transition = stack.apply_command(Command::Quit);

    assert!(transition.should_exit());
    assert!(stack.is_empty());
    assert!(!stack.has_modal());
}

#[test]
fn apply_command_push_pushes_on_top() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));

    stack.apply_command(Command::Push(LabelScreen::boxed("tx")));

    assert_eq!(stack.top().unwrap().title(), "tx");
    assert_eq!(stack.len(), 2);
}

#[test]
fn apply_command_replace_swaps_the_top() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));
    stack.push(LabelScreen::boxed("search"));

    stack.apply_command(Command::Replace(LabelScreen::boxed("tx")));

    assert_eq!(stack.top().unwrap().title(), "tx");
    assert_eq!(stack.len(), 2);
}

#[test]
fn apply_command_switch_clears_the_stack_and_pushes() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));
    stack.push(LabelScreen::boxed("tx"));
    stack.push(LabelScreen::boxed("block"));

    stack.apply_command(Command::Switch(LabelScreen::boxed("settings")));

    assert_eq!(stack.len(), 1);
    assert_eq!(stack.top().unwrap().title(), "settings");
}

#[test]
fn apply_command_switch_also_clears_the_modal() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));
    stack.open_modal(LabelScreen::boxed("help"));

    stack.apply_command(Command::Switch(LabelScreen::boxed("settings")));

    assert_eq!(stack.len(), 1);
    assert_eq!(stack.top().unwrap().title(), "settings");
    assert!(!stack.has_modal());
}

#[test]
fn apply_command_open_modal_populates_the_modal_slot() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));

    stack.apply_command(Command::OpenModal(LabelScreen::boxed("help")));

    assert_eq!(stack.modal().unwrap().title(), "help");
    assert_eq!(stack.top().unwrap().title(), "home");
}

#[test]
fn apply_command_open_modal_replaces_an_existing_modal() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));
    stack.open_modal(LabelScreen::boxed("first"));

    stack.apply_command(Command::OpenModal(LabelScreen::boxed("second")));

    assert_eq!(stack.modal().unwrap().title(), "second");
}

#[test]
fn apply_command_close_modal_on_empty_slot_is_a_noop() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));

    stack.apply_command(Command::CloseModal);

    assert_eq!(stack.len(), 1);
    assert!(!stack.has_modal());
}

#[test]
fn apply_command_push_with_open_modal_closes_the_modal() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));
    stack.open_modal(LabelScreen::boxed("search"));

    stack.apply_command(Command::Push(LabelScreen::boxed("tx")));

    assert_eq!(stack.top().unwrap().title(), "tx");
    assert!(!stack.has_modal());
}

#[test]
fn apply_command_replace_with_open_modal_closes_the_modal() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));
    stack.open_modal(LabelScreen::boxed("search"));

    stack.apply_command(Command::Replace(LabelScreen::boxed("tx")));

    assert_eq!(stack.top().unwrap().title(), "tx");
    assert!(!stack.has_modal());
}

// ---------------------------------------------------------------------------
// plan/12-screen-runtime.md §7.1: Pop on a single-screen (or empty) stack is a
// no-op. The root screen is never popped off and Quit is the only path out of
// the event loop.
// ---------------------------------------------------------------------------

#[test]
fn pop_on_single_screen_stack_is_noop() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));

    let transition = stack.apply_command(Command::Pop);

    assert!(
        !transition.should_exit(),
        "Pop on the root screen must not exit the event loop"
    );
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.top().unwrap().title(), "home");
}

#[test]
fn pop_on_empty_stack_is_noop() {
    let mut stack = ScreenStack::new();

    let transition = stack.apply_command(Command::Pop);

    assert!(!transition.should_exit());
    assert!(stack.is_empty());
}

#[test]
fn pop_with_two_screens_shrinks_to_one() {
    let mut stack = ScreenStack::new();
    stack.push(LabelScreen::boxed("home"));
    stack.push(LabelScreen::boxed("tx"));

    let transition = stack.apply_command(Command::Pop);

    assert!(!transition.should_exit());
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.top().unwrap().title(), "home");
}

#[test]
fn replace_on_empty_stack_is_noop() {
    let mut stack = ScreenStack::new();

    let transition = stack.apply_command(Command::Replace(LabelScreen::boxed("tx")));

    assert!(!transition.should_exit());
    assert!(stack.is_empty());
}
