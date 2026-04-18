//! Wall-clock-backed `Clock` adapter.
//!
//! The only production implementation of [`crate::application::ports::Clock`].
//! Tests use `FrozenClock` under `tests/support/stubs.rs`.
//!
//! See `plan/2-search.md` section 12.5.

use std::time::Instant;

use crate::application::ports::Clock;

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl SystemClock {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}
