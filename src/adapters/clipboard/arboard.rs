//! `arboard`-backed `ClipboardPort` adapter.
//!
//! Runs live against `arboard::Clipboard`. Degrades to a no-op when
//! the underlying handle cannot be built — no DISPLAY, no Wayland
//! socket, headless CI box. See `plan/17-navigable-values.md` §5.1.

use std::sync::{Arc, Mutex};

use crate::{application::ports::ClipboardPort, domain::DomainError};

/// Boxed constructor for an `arboard::Clipboard` handle. Injected to
/// simulate headless-init failures in unit tests without requiring a
/// running display.
pub type ClipboardFactory =
    Box<dyn Fn() -> Result<::arboard::Clipboard, ::arboard::Error> + Send + Sync>;

/// Production `ClipboardPort` backed by `arboard`.
///
/// Holds its handle inside an `Arc<Mutex<_>>` so the adapter can be
/// cloned across screens without forcing a `Clone` bound on the
/// underlying `arboard::Clipboard`.
pub struct ArboardClipboard {
    inner: Arc<Mutex<Option<::arboard::Clipboard>>>,
}

impl ArboardClipboard {
    /// Build a live adapter. Logs a single warning and degrades to a
    /// no-op when `arboard::Clipboard::new()` fails (no DISPLAY, no
    /// Wayland, etc.). The adapter is infallible by construction.
    #[must_use]
    pub fn new() -> Self {
        Self::with_factory(Box::new(::arboard::Clipboard::new))
    }

    /// Same as [`Self::new`] but lets callers inject the clipboard
    /// factory. Used by unit tests to exercise the graceful-fallback
    /// path without needing a headless display.
    #[must_use]
    pub fn with_factory(factory: ClipboardFactory) -> Self {
        let handle = match factory() {
            Ok(cb) => Some(cb),
            Err(err) => {
                tracing::warn!(
                    target: "blockexplorer_tui::clipboard",
                    "arboard::Clipboard::new() failed ({err}); \
                     clipboard copies will be silently dropped until \
                     the environment provides a clipboard."
                );
                None
            }
        };
        Self {
            inner: Arc::new(Mutex::new(handle)),
        }
    }

    /// `true` when the adapter holds a live handle. `false` after a
    /// graceful fallback. Exposed so tests can assert on the degraded
    /// state without reaching into private fields.
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.inner.lock().map(|g| g.is_some()).unwrap_or(false)
    }
}

impl Default for ArboardClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ArboardClipboard {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl ClipboardPort for ArboardClipboard {
    fn set(&self, text: &str) -> Result<(), DomainError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| DomainError::Internal("arboard mutex poisoned".to_string()))?;
        let Some(cb) = guard.as_mut() else {
            // Graceful no-op: the adapter already warned at
            // construction time, the caller just wanted the value
            // to land somewhere.
            return Ok(());
        };
        if let Err(err) = cb.set_text(text.to_string()) {
            tracing::warn!(
                target: "blockexplorer_tui::clipboard",
                "arboard set_text failed ({err}); swallowed to keep the TUI alive",
            );
        }
        Ok(())
    }
}
