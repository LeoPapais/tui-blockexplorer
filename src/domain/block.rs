//! Block-level value objects.

use crate::domain::{DomainError, tx};

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

/// A 32-byte block hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockHash([u8; 32]);

impl BlockHash {
    /// Parse a `0x`-prefixed hex string into a `BlockHash`.
    pub fn from_hex(s: &str) -> Result<Self, DomainError> {
        tx::parse_hash32(s, "block hash").map(Self)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(66);
        out.push_str("0x");
        out.push_str(&hex::encode(self.0));
        out
    }
}

/// Lightweight summary used by `BlockLookupPort`. The full `Block`
/// entity lives in `plan/3-block-detail.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSummary {
    pub number: BlockNumber,
    pub hash: BlockHash,
}
