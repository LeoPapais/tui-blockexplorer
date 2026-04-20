//! Universal field cursor.
//!
//! Every detail screen owns a `FieldCursor` that moves across its
//! navigable values. `y` copies the value via `ClipboardPort`,
//! `Enter` pushes the next screen through `NavigationFactory`. See
//! `plan/17-navigable-values.md` §4.

use std::sync::Arc;

use crate::{
    adapters::ui::screen::Screen,
    application::ports::ClipboardPort,
    domain::{Chain, NavigableValue},
};

/// One selectable value on the current screen. `label` is human-
/// facing only (tests assert on it to pin ordering).
#[derive(Debug, Clone)]
pub struct FieldEntry {
    pub value: NavigableValue,
    pub label: &'static str,
}

impl FieldEntry {
    #[must_use]
    pub const fn new(label: &'static str, value: NavigableValue) -> Self {
        Self { label, value }
    }
}

/// Direction a key press asked the cursor to move in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorDir {
    Left,
    Right,
    Up,
    Down,
}

/// State machine for the field cursor.
#[derive(Debug, Clone, Copy, Default)]
pub struct FieldCursor {
    active: Option<usize>,
}

impl FieldCursor {
    #[must_use]
    pub const fn new() -> Self {
        Self { active: None }
    }

    /// `true` once the user pressed the first arrow key.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Index of the active field, or `None` when inactive.
    #[must_use]
    pub const fn active(&self) -> Option<usize> {
        self.active
    }

    /// Release the cursor without popping the screen. Called by `Esc`
    /// when the cursor is the top-of-stack input target.
    pub fn deactivate(&mut self) {
        self.active = None;
    }

    /// Move the cursor within a list of `len` entries. An inactive
    /// cursor activates at index 0 regardless of direction. Motion
    /// saturates at both edges (wrap-around is intentionally a no-op,
    /// see `plan/17-navigable-values.md` §4).
    pub fn move_in(&mut self, len: usize, dir: CursorDir) {
        if len == 0 {
            self.active = None;
            return;
        }
        let new = match (self.active, dir) {
            (None, _) => 0,
            (Some(i), CursorDir::Right | CursorDir::Down) => {
                let next = i + 1;
                if next >= len { i } else { next }
            }
            (Some(i), CursorDir::Left | CursorDir::Up) => i.saturating_sub(1),
        };
        self.active = Some(new.min(len.saturating_sub(1)));
    }

    /// Return the field currently under the cursor.
    #[must_use]
    pub fn current<'a>(&self, fields: &'a [FieldEntry]) -> Option<&'a FieldEntry> {
        self.active.and_then(|i| fields.get(i))
    }
}

/// Factory that translates a navigable value into the next screen to
/// push. Every adapter-side screen needs this to react to `Enter`.
///
/// Production implementation lives in `src/infra/navigate.rs`; tests
/// use `StubNavigationFactory` in `tests/support/stubs.rs`.
pub trait NavigationFactory: Send + Sync {
    fn open(&self, value: &NavigableValue, active_chain: Chain) -> Option<Box<dyn Screen>>;
}

impl<F> NavigationFactory for F
where
    F: Fn(&NavigableValue, Chain) -> Option<Box<dyn Screen>> + Send + Sync,
{
    fn open(&self, value: &NavigableValue, active_chain: Chain) -> Option<Box<dyn Screen>> {
        (self)(value, active_chain)
    }
}

/// Handle bundling every injected collaborator the cursor needs.
/// Screens hold an `Option<CursorServices>`; `None` keeps the cursor
/// copy/navigation inert (matches the default demo-mode boot).
#[derive(Clone)]
pub struct CursorServices {
    pub clipboard: Arc<dyn ClipboardPort>,
    pub nav: Arc<dyn NavigationFactory>,
    pub chain: Chain,
}

impl CursorServices {
    #[must_use]
    pub fn new(
        clipboard: Arc<dyn ClipboardPort>,
        nav: Arc<dyn NavigationFactory>,
        chain: Chain,
    ) -> Self {
        Self {
            clipboard,
            nav,
            chain,
        }
    }

    /// Ship the cursor's current value to the clipboard. Silently
    /// ignores clipboard errors — the TUI stays alive even when the
    /// adapter degraded to a no-op.
    pub fn copy(&self, value: &NavigableValue) {
        let _ = self.clipboard.set(&value.copy_text());
    }

    /// Ask the factory for the next screen. Returns `None` for
    /// copy-only values or when the factory has no mapping.
    #[must_use]
    pub fn open(&self, value: &NavigableValue) -> Option<Box<dyn Screen>> {
        if !value.can_navigate() {
            return None;
        }
        self.nav.open(value, self.chain)
    }
}
