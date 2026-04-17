//! Screen trait, stack command enum and the `ScreenStack` container.
//!
//! See `plan/12-screen-runtime.md` sections 2 and 3.

use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

/// Result of an input handler or tick on a screen. Consumed by the
/// runtime dispatcher to decide whether to keep going, pop the current
/// screen or terminate the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// No state change.
    None,
    /// Pop the current screen. If the stack becomes empty, the runtime
    /// exits cleanly.
    Pop,
    /// Exit the process right away.
    Quit,
    /// Ask the screen to refresh its data on its next tick.
    Refresh,
}

/// A screen renders itself into a `ratatui::Frame` and receives user
/// input. Implementations live under `src/adapters/ui/*`.
pub trait Screen: Send {
    /// Short name used on the breadcrumb and for logging.
    fn title(&self) -> &str;

    /// Draw the screen at `area` within `frame`.
    fn render(&self, frame: &mut Frame<'_>, area: Rect);

    /// Handle a key event. Returning [`Command::Quit`] terminates the
    /// runtime; [`Command::Pop`] pops this screen from the stack.
    fn handle_key(&mut self, key: KeyEvent) -> Command;

    /// Called by the runtime on a periodic tick. Default implementation
    /// does nothing so simple screens do not have to implement it.
    fn tick(&mut self) -> Command {
        Command::None
    }
}

/// Push-down stack of screens. The runtime always renders the top of the
/// stack.
#[derive(Default)]
pub struct ScreenStack {
    screens: Vec<Box<dyn Screen>>,
}

impl ScreenStack {
    /// Create an empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a new screen on top.
    pub fn push(&mut self, screen: Box<dyn Screen>) {
        self.screens.push(screen);
    }

    /// Pop the top screen, returning it.
    pub fn pop(&mut self) -> Option<Box<dyn Screen>> {
        self.screens.pop()
    }

    /// Read-only reference to the top screen.
    #[must_use]
    pub fn top(&self) -> Option<&(dyn Screen + 'static)> {
        self.screens.last().map(AsRef::as_ref)
    }

    /// Mutable reference to the top screen.
    pub fn top_mut(&mut self) -> Option<&mut (dyn Screen + 'static)> {
        self.screens.last_mut().map(AsMut::as_mut)
    }

    /// True when the stack holds no screens; the runtime uses this to
    /// decide when to exit.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.screens.is_empty()
    }

    /// Number of screens on the stack.
    #[must_use]
    pub fn len(&self) -> usize {
        self.screens.len()
    }

    /// Clear every screen. Called in response to [`Command::Quit`].
    pub fn clear(&mut self) {
        self.screens.clear();
    }
}
