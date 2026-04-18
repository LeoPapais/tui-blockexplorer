//! `Clock` port — abstracts over `Instant::now()` so application and
//! adapter code can be driven with a stub in tests.
//!
//! See `plan/2-search.md` section 12.5 and the §8.12 item in
//! `plan/15-backlog.md`.

use std::time::Instant;

/// Source of monotonic time.
///
/// Implementations must be cheap to call and `Send + Sync` so a single
/// clock can be shared by many tasks. See
/// `plan/2-search.md` §12.5.
pub trait Clock: Send + Sync {
    /// Monotonic "now".
    fn now(&self) -> Instant;
}

impl<C: Clock + ?Sized> Clock for &C {
    fn now(&self) -> Instant {
        (*self).now()
    }
}

impl<C: Clock + ?Sized> Clock for std::sync::Arc<C> {
    fn now(&self) -> Instant {
        (**self).now()
    }
}
