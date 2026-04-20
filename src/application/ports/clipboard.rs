//! `ClipboardPort` — outbound port used by the TUI to push a string
//! to the OS clipboard on `y`.
//!
//! The TUI never reads from the clipboard. Implementations are
//! allowed to degrade to a no-op on headless systems as long as they
//! keep the `Ok(())` contract (see
//! `plan/17-navigable-values.md` §3 and §5.1).

use crate::domain::DomainError;

/// Write `text` to the OS clipboard.
///
/// Implementations must be `Send + Sync` so a single adapter can be
/// shared across every screen through an `Arc`. Failing to reach the
/// clipboard (no `DISPLAY`, no Wayland socket, permission denied,
/// ...) should NOT crash the TUI; the live adapter logs and returns
/// `Ok(())` in that case. Returning an error is reserved for genuinely
/// unexpected conditions (for example a poisoned lock) that callers
/// may want to surface in a modal.
pub trait ClipboardPort: Send + Sync {
    fn set(&self, text: &str) -> Result<(), DomainError>;
}

impl<C: ClipboardPort + ?Sized> ClipboardPort for &C {
    fn set(&self, text: &str) -> Result<(), DomainError> {
        (*self).set(text)
    }
}

impl<C: ClipboardPort + ?Sized> ClipboardPort for std::sync::Arc<C> {
    fn set(&self, text: &str) -> Result<(), DomainError> {
        (**self).set(text)
    }
}
