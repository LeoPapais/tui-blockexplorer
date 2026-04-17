//! Block-level value objects.

use crate::domain::{Address, Chain, DomainError, TxHash, UnixTimestamp, Wei, tx};

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

/// Lightweight summary used by `BlockLookupPort`. Contains just
/// enough to disambiguate a block in the Search candidate list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSummary {
    pub number: BlockNumber,
    pub hash: BlockHash,
}

/// Addressing mode for the full-block reader port. Search first,
/// detail second: Search hands in a BlockSummary, the detail screen
/// asks `BlockReaderPort` to fetch the full thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockId {
    Number(BlockNumber),
    Hash(BlockHash),
}

impl From<BlockNumber> for BlockId {
    fn from(n: BlockNumber) -> Self {
        BlockId::Number(n)
    }
}

impl From<BlockHash> for BlockId {
    fn from(h: BlockHash) -> Self {
        BlockId::Hash(h)
    }
}

/// Full block entity used by the BlockDetail screen. See
/// `plan/3-block-detail.md` section 11.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub chain: Chain,
    pub number: BlockNumber,
    pub hash: BlockHash,
    pub parent_hash: BlockHash,
    pub timestamp: UnixTimestamp,
    pub miner: Address,
    pub gas_used: u64,
    pub gas_limit: u64,
    /// `None` on pre-1559 chains or the odd L2 that does not surface
    /// a base fee. The UI falls back to "-" in that case.
    pub base_fee: Option<Wei>,
    pub size: u64,
    pub extra_data: Vec<u8>,
    pub tx_hashes: Vec<TxHash>,
}

impl Block {
    /// Convenience helper for tests: build an id pointing back at
    /// this block by number.
    #[must_use]
    pub fn id_by_number(&self) -> BlockId {
        BlockId::Number(self.number)
    }

    /// Convenience helper for tests: build an id pointing back at
    /// this block by hash.
    #[must_use]
    pub fn id_by_hash(&self) -> BlockId {
        BlockId::Hash(self.hash)
    }
}
