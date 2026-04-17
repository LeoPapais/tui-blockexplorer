//! Screen trait, stack command enum and the `ScreenStack` container.
//!
//! See `plan/12-screen-runtime.md` sections 2 and 3.

use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

/// Result of an input handler or tick on a screen. Consumed by the
/// runtime dispatcher to decide whether to keep going, pop the current
/// screen, push a new one or terminate the process.
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
    /// Push a new screen on top of the stack.
    Push(Box<dyn Screen>),
    /// Pop the current screen and push `0` on top atomically. Useful
    /// when a modal-ish screen wants to hand control to a detail page.
    Replace(Box<dyn Screen>),
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::None => write!(f, "Command::None"),
            Command::Pop => write!(f, "Command::Pop"),
            Command::Quit => write!(f, "Command::Quit"),
            Command::Refresh => write!(f, "Command::Refresh"),
            Command::Push(s) => write!(f, "Command::Push({})", s.title()),
            Command::Replace(s) => write!(f, "Command::Replace({})", s.title()),
        }
    }
}

impl PartialEq for Command {
    fn eq(&self, other: &Self) -> bool {
        // Only compare the unit variants; composite variants are
        // never equal to anything since they carry boxed trait
        // objects without an intrinsic equality.
        matches!(
            (self, other),
            (Command::None, Command::None)
                | (Command::Pop, Command::Pop)
                | (Command::Quit, Command::Quit)
                | (Command::Refresh, Command::Refresh)
        )
    }
}

impl Eq for Command {}

/// A screen renders itself into a `ratatui::Frame` and receives user
/// input. Implementations live under `src/adapters/ui/*`.
///
/// Screens must be `'static` so the runtime can keep them on a
/// heterogeneous stack and so tests can downcast through
/// [`Screen::as_any`].
pub trait Screen: Send + 'static {
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

    /// Expose the screen as `Any` so tests can downcast to the
    /// concrete type. Real runtime code never uses this.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Mutable `Any` counterpart of [`Screen::as_any`].
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
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
