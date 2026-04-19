//! Dispatcher-level global key map.
//!
//! The runtime consults this map *before* delegating a key event to
//! the current screen. It is the place where bindings that must work
//! on every screen live, namely:
//!
//! - `/` opens a universal search modal,
//! - `?` opens the help modal.
//!
//! Factories are closures so `infra` can lazily build the concrete
//! screens (Search needs a live feed + RPC client; Help is built from
//! the active screen's hints). A `None` factory means the binding is
//! unregistered and the key is left alone.
//!
//! See `plan/12-screen-runtime.md` §7 items 4 and 5.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::adapters::ui::screen::{Command, Screen};

/// Boxed factory that produces a fresh modal screen on demand.
pub type ModalFactory = Box<dyn Fn() -> Box<dyn Screen> + Send + 'static>;

/// Declarative map from a global key to a modal-opening command.
///
/// Populated by `infra` at composition time and threaded into
/// `run_event_loop`. The default map has no entries — useful for
/// tests that want to isolate a single screen.
#[derive(Default)]
pub struct GlobalKeyMap {
    on_slash: Option<ModalFactory>,
    on_question_mark: Option<ModalFactory>,
}

impl GlobalKeyMap {
    /// Register a factory for `/` that opens a search-like modal.
    #[must_use]
    pub fn with_search_factory<F>(mut self, factory: F) -> Self
    where
        F: Fn() -> Box<dyn Screen> + Send + 'static,
    {
        self.on_slash = Some(Box::new(factory));
        self
    }

    /// Register a factory for `?` that opens the help modal.
    #[must_use]
    pub fn with_help_factory<F>(mut self, factory: F) -> Self
    where
        F: Fn() -> Box<dyn Screen> + Send + 'static,
    {
        self.on_question_mark = Some(Box::new(factory));
        self
    }

    /// Resolve the given key. Returns [`Command::None`] when the key
    /// is not claimed by the map so callers can forward it to the
    /// top screen.
    #[must_use]
    pub fn dispatch(&self, key: KeyEvent) -> Command {
        // Only plain presses trigger the global bindings. Ctrl+/,
        // Shift+? and friends are deliberately excluded so terminals
        // with heavy modifier mapping do not hijack screen bindings.
        if key.modifiers != KeyModifiers::NONE && key.modifiers != KeyModifiers::SHIFT {
            return Command::None;
        }
        match key.code {
            KeyCode::Char('/') => self
                .on_slash
                .as_ref()
                .map_or(Command::None, |f| Command::OpenModal(f())),
            KeyCode::Char('?') => self
                .on_question_mark
                .as_ref()
                .map_or(Command::None, |f| Command::OpenModal(f())),
            _ => Command::None,
        }
    }
}
