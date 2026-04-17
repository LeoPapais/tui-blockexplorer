//! Unix timestamp value object.
//!
//! Minimal for now: just a newtype around `u64` seconds. When the Home
//! / Block screens grow a relative-time formatter we will add a
//! `Clock` port and move the "X ago" logic into the application
//! layer.

/// Seconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnixTimestamp(u64);

impl UnixTimestamp {
    #[must_use]
    pub const fn from_seconds(secs: u64) -> Self {
        Self(secs)
    }

    #[must_use]
    pub const fn seconds(self) -> u64 {
        self.0
    }
}

impl From<u64> for UnixTimestamp {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
