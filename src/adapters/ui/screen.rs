//! Screen trait, stack command enum and the `ScreenStack` container.
//!
//! See `plan/12-screen-runtime.md` sections 2 and 7.

use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

/// Result of an input handler or tick on a screen. Consumed by the
/// runtime dispatcher to decide whether to keep going, pop the current
/// screen, push a new one, open or close a modal or terminate the
/// process.
pub enum Command {
    /// No state change.
    None,
    /// Pop the current screen. If a modal is open the dispatcher will
    /// close the modal instead; see [`ScreenStack::apply_command`]. If
    /// the back stack becomes empty, the runtime exits cleanly.
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
    /// Clear the whole stack (and any open modal) and push the new
    /// screen as the only one. Used for global jumps like `gh`, `gs`,
    /// `gm` so the back stack never drags context across top-level
    /// views. See `plan/12-screen-runtime.md` §7.
    Switch(Box<dyn Screen>),
    /// Open the given screen as a modal on top of the current stack
    /// without pushing onto it. Subsequent key events flow to the
    /// modal first. See `plan/12-screen-runtime.md` §7.
    OpenModal(Box<dyn Screen>),
    /// Dismiss the currently open modal, if any. A no-op when no
    /// modal is open.
    CloseModal,
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
            Command::Switch(s) => write!(f, "Command::Switch({})", s.title()),
            Command::OpenModal(s) => write!(f, "Command::OpenModal({})", s.title()),
            Command::CloseModal => write!(f, "Command::CloseModal"),
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
                | (Command::CloseModal, Command::CloseModal)
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

/// Dispatcher transition reported by [`ScreenStack::apply_command`].
/// Tells the caller whether the event loop should keep running or
/// drop out cleanly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// Keep going: redraw and wait for the next event.
    Continue,
    /// Exit the event loop (user pressed `q`, last screen popped,
    /// Ctrl+C received, ...).
    Exit,
}

impl Transition {
    /// `true` when the dispatcher should leave the event loop.
    #[must_use]
    pub const fn should_exit(self) -> bool {
        matches!(self, Transition::Exit)
    }
}

/// Push-down stack of screens. The runtime always renders the top of the
/// stack, optionally overlaid with a modal from the dedicated
/// [`ScreenStack::modal`] slot.
#[derive(Default)]
pub struct ScreenStack {
    screens: Vec<Box<dyn Screen>>,
    modal: Option<Box<dyn Screen>>,
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

    /// Clear every screen and any modal. Called in response to
    /// [`Command::Quit`] or [`Command::Switch`].
    pub fn clear(&mut self) {
        self.screens.clear();
        self.modal = None;
    }

    /// Open the given screen as a modal. Replaces any currently open
    /// modal.
    pub fn open_modal(&mut self, modal: Box<dyn Screen>) {
        self.modal = Some(modal);
    }

    /// Close the currently open modal, if any. Returns the modal if
    /// one was open.
    pub fn close_modal(&mut self) -> Option<Box<dyn Screen>> {
        self.modal.take()
    }

    /// `true` when a modal is currently open.
    #[must_use]
    pub fn has_modal(&self) -> bool {
        self.modal.is_some()
    }

    /// Read-only reference to the currently open modal, if any.
    #[must_use]
    pub fn modal(&self) -> Option<&(dyn Screen + 'static)> {
        self.modal.as_deref()
    }

    /// Mutable reference to the currently open modal, if any.
    pub fn modal_mut(&mut self) -> Option<&mut (dyn Screen + 'static)> {
        self.modal.as_deref_mut()
    }

    /// Apply a [`Command`] to the stack. Returns a [`Transition`] the
    /// caller consults to decide whether to keep running the event
    /// loop. Shared by `src/infra/runtime.rs` and the BDD harness so
    /// the two never drift.
    pub fn apply_command(&mut self, cmd: Command) -> Transition {
        match cmd {
            Command::None | Command::Refresh => Transition::Continue,
            Command::Pop => {
                // A Pop while a modal is open always closes the modal
                // first, so `Esc` / `q` inside a modal does not pop the
                // underlying screen.
                if self.modal.is_some() {
                    self.modal = None;
                    Transition::Continue
                } else {
                    self.screens.pop();
                    if self.screens.is_empty() {
                        Transition::Exit
                    } else {
                        Transition::Continue
                    }
                }
            }
            Command::Quit => {
                self.clear();
                Transition::Exit
            }
            Command::Push(screen) => {
                // Navigation dismisses any open modal so the new top
                // is rendered without the stale overlay. Matches the
                // way browsers drop ephemeral popovers on navigation.
                self.modal = None;
                self.screens.push(screen);
                Transition::Continue
            }
            Command::Replace(screen) => {
                self.modal = None;
                self.screens.pop();
                self.screens.push(screen);
                Transition::Continue
            }
            Command::Switch(screen) => {
                self.clear();
                self.screens.push(screen);
                Transition::Continue
            }
            Command::OpenModal(screen) => {
                self.modal = Some(screen);
                Transition::Continue
            }
            Command::CloseModal => {
                self.modal = None;
                Transition::Continue
            }
        }
    }
}
