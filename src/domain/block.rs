//! Block-level value objects.

use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use tiny_keccak::{Hasher, Keccak};

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
    /// Validator that sealed this block, recovered from `extraData`
    /// when the chain uses a signer-in-extraData consensus (Polygon
    /// PoS / Bor). `None` on chains where `miner` is already the real
    /// author or when recovery failed.
    ///
    /// See `plan/15-backlog.md` section 3.5.
    pub extra_signer: Option<Address>,
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

/// Recover the Polygon PoS validator that sealed a block.
///
/// On Bor-based Polygon PoS the `miner` field is the zero address and
/// the real author is recovered by `ecrecover` over the block's seal
/// hash, using the last 65 bytes of `extra_data` as the signature.
/// The preceding bytes (the "vanity") are ignored.
///
/// The seal hash itself is keccak256 of the RLP-encoded header with
/// the signature stripped from `extraData`; it is a responsibility of
/// the caller. This function is pure, domain-layer math and does not
/// reconstruct the header.
///
/// Returns `None` when `extra_data` is shorter than the 65-byte
/// signature, when the signature bytes are malformed, or when the
/// recovery id is outside the accepted range. See
/// `plan/15-backlog.md` section 3.5.
#[must_use]
pub fn recover_polygon_signer(extra_data: &[u8], seal_hash: &[u8; 32]) -> Option<Address> {
    if extra_data.len() < 65 {
        return None;
    }
    let sig_bytes = &extra_data[extra_data.len() - 65..];
    let (rs, v) = sig_bytes.split_at(64);
    let v = v[0];
    // Bor stores recid as 0 / 1; accept 27 / 28 for parity with
    // go-ethereum's ecrecover helper.
    let recid_byte = match v {
        0 | 1 => v,
        27 | 28 => v - 27,
        _ => return None,
    };
    let signature = Signature::from_slice(rs).ok()?;
    let recid = RecoveryId::try_from(recid_byte).ok()?;
    let verifying_key = VerifyingKey::recover_from_prehash(seal_hash, &signature, recid).ok()?;
    Some(address_from_verifying_key(&verifying_key))
}

fn address_from_verifying_key(vk: &VerifyingKey) -> Address {
    let encoded = vk.to_encoded_point(false);
    let pk = encoded.as_bytes();
    // `to_encoded_point(false)` always returns [0x04, X (32), Y (32)].
    debug_assert_eq!(pk.len(), 65);
    debug_assert_eq!(pk[0], 0x04);
    let mut hasher = Keccak::v256();
    hasher.update(&pk[1..]);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&out[12..]);
    Address::from_bytes(addr)
}
