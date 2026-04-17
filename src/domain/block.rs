//! Block-level value objects.
//!
//! Only `BlockNumber` is modelled at this phase; the richer `Block` entity
//! will land with `plan/3-block-detail.md`.

/// A block height. Wrapped in a newtype to avoid mixing with other `u64`
/// quantities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockNumber(u64);

impl BlockNumber {
    /// Build a new block number.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Extract the raw block height.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl From<u64> for BlockNumber {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
