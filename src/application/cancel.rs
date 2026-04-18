//! A trivial cancellation flag shared across the application layer.
//!
//! Use cases that accept long-running work (pagination, receipts,
//! tracing) take a `&CancelFlag`. When the caller flips the flag
//! the use case bails out at the earliest checkpoint without
//! surfacing an error variant — the typical reason is "the UI
//! moved on", not a failure.
//!
//! Introduced to support the cancellation hook documented in
//! `plan/3-block-detail.md` §12.3. Intentionally stdlib-only so we
//! do not pull `tokio_util` for a feature this modest.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Cheaply-cloneable cancellation flag. Clones share the same
/// underlying atomic: cancelling any handle cancels all of them.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag {
    flag: Arc<AtomicBool>,
}

impl CancelFlag {
    /// Build a fresh, uncancelled flag.
    #[must_use]
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Flip the flag to "cancelled". Subsequent `is_cancelled`
    /// calls — including ones already waiting inside a use case —
    /// will see the new state.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
    }

    /// Read the current state.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
}
