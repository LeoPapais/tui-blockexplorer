//! Block-level value objects.

use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use tiny_keccak::{Hasher, Keccak};

use crate::domain::{Address, Chain, DomainError, TxHash, TxStatus, UnixTimestamp, Wei, tx};

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
    /// Post-Shanghai withdrawals attached to the block. Empty on
    /// pre-Shanghai Ethereum, chains that do not implement
    /// EIP-4895, or when the adapter pointedly left the array
    /// untouched. See `plan/3-block-detail.md` §12.5.
    pub withdrawals: Vec<Withdrawal>,
}

/// One EIP-4895 withdrawal carried by a post-Shanghai Ethereum
/// block. Produced by the block reader and consumed by
/// `load_block_withdrawals`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Withdrawal {
    /// Monotonic index assigned by the consensus layer.
    pub index: u64,
    /// Validator that earned the withdrawal.
    pub validator_index: u64,
    /// Recipient address.
    pub address: Address,
    /// Amount withdrawn, in gwei (EIP-4895 uses gwei directly, not
    /// wei).
    pub amount_gwei: u64,
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

/// High-level category rendered next to each row on the Block Detail
/// Transactions tab. See `plan/3-block-detail.md` §12.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TxCategory {
    /// Plain value transfer: `to` is set, `input` is empty and the
    /// transaction carries a non-zero value.
    Transfer,
    /// Contract deployment: `to` is `None` (the EVM creates the new
    /// contract address from the sender + nonce).
    Deploy,
    /// Everything else: method calls, zero-value calls, approvals,
    /// system calls on L2s, ...
    Interaction,
}

impl TxCategory {
    /// Classify a transaction from the three fields every receipt
    /// and block-full-tx response carries.
    ///
    /// The order mirrors the plan:
    ///
    /// - `to == None`                         -> [`Self::Deploy`].
    /// - empty `input` && non-zero `value`    -> [`Self::Transfer`].
    /// - anything else                        -> [`Self::Interaction`].
    #[must_use]
    pub fn classify(to: Option<Address>, input: &[u8], value: Wei) -> Self {
        if to.is_none() {
            return Self::Deploy;
        }
        if input.is_empty() && value.value() > 0 {
            return Self::Transfer;
        }
        Self::Interaction
    }

    /// Short, terminal-friendly badge shown on the Transactions tab.
    #[must_use]
    pub const fn badge(self) -> &'static str {
        match self {
            Self::Transfer => "T",
            Self::Deploy => "D",
            Self::Interaction => "I",
        }
    }

    /// Human label used by tests, logs and future tooltips.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Transfer => "transfer",
            Self::Deploy => "deploy",
            Self::Interaction => "interaction",
        }
    }
}

/// One row on the Block Detail "Transactions" tab. Produced by the
/// application-layer `load_block_transactions` use case from the
/// `BlockReceiptsPort` response. See `plan/3-block-detail.md` §12.3.
///
/// The shape combines just enough receipt fields (`gas_used`,
/// `status`, `contract_address`) with just enough tx fields
/// (`value`, `has_calldata`) to render a dense single-line row
/// per transaction while keeping the transport cheap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTxReceipt {
    pub hash: TxHash,
    pub tx_index: u64,
    pub from: Address,
    /// `None` on contract-creation transactions (see also
    /// `contract_address`).
    pub to: Option<Address>,
    pub value: Wei,
    /// True when the original tx carried any calldata past the
    /// `0x` prefix. Stored as a boolean so the adapter does not
    /// need to ship kilobytes of input for every tx row.
    pub has_calldata: bool,
    /// `None` while the receipt is still missing (should never
    /// happen for confirmed block txs, kept optional for
    /// defensive mapping).
    pub gas_used: Option<u64>,
    pub status: TxStatus,
    /// Set when the tx deployed a new contract. Equal to the
    /// receipt's `contractAddress` field.
    pub contract_address: Option<Address>,
    /// Precomputed through [`TxCategory::classify`] so the UI can
    /// render the badge without re-deriving it each frame.
    pub category: TxCategory,
}

impl BlockTxReceipt {
    /// Build a row from the raw fields exposed by
    /// `eth_getBlockByNumber(fullTxs=true)` + `eth_getBlockReceipts`.
    /// Computes `has_calldata` from `input` and the category via
    /// [`TxCategory::classify`].
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_fields(
        hash: TxHash,
        tx_index: u64,
        from: Address,
        to: Option<Address>,
        value: Wei,
        input: &[u8],
        gas_used: Option<u64>,
        status: TxStatus,
        contract_address: Option<Address>,
    ) -> Self {
        let has_calldata = !input.is_empty();
        let category = TxCategory::classify(to, input, value);
        Self {
            hash,
            tx_index,
            from,
            to,
            value,
            has_calldata,
            gas_used,
            status,
            contract_address,
            category,
        }
    }
}

/// Pagination cursor used by `load_block_transactions`. Cursor-based
/// so callers never need to know the full row count up front.
///
/// - `offset` is the 0-indexed row to start at.
/// - `page_size` is the number of rows to return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockTxCursor {
    pub offset: usize,
    pub page_size: usize,
}

impl BlockTxCursor {
    /// Default cursor used by the Transactions tab: first page,
    /// 20 rows.
    #[must_use]
    pub const fn first_page() -> Self {
        Self {
            offset: 0,
            page_size: 20,
        }
    }

    /// Cursor pointing at the page immediately after the current
    /// page. Clamped at the page size defined by `self`.
    #[must_use]
    pub const fn next(self) -> Self {
        Self {
            offset: self.offset + self.page_size,
            page_size: self.page_size,
        }
    }
}

impl Default for BlockTxCursor {
    fn default() -> Self {
        Self::first_page()
    }
}

/// One page of `BlockTxReceipt` rows, carrying enough metadata for
/// the screen to know whether another page is available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTxPage {
    pub rows: Vec<BlockTxReceipt>,
    /// Cursor to pass to the next call to `load_block_transactions`;
    /// `None` when the current page reached the end of the block.
    pub next: Option<BlockTxCursor>,
    /// Total number of rows in the underlying block. Exposed so the
    /// UI can render "showing 20 of 312" without a second call.
    pub total: usize,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_to() -> Address {
        Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
    }

    #[test]
    fn classifies_contract_creation_as_deploy() {
        assert_eq!(
            TxCategory::classify(None, &[0x60, 0x80], Wei::new(0)),
            TxCategory::Deploy,
        );
    }

    #[test]
    fn classifies_empty_calldata_with_value_as_transfer() {
        assert_eq!(
            TxCategory::classify(Some(sample_to()), &[], Wei::new(1_000)),
            TxCategory::Transfer,
        );
    }

    #[test]
    fn classifies_zero_value_empty_calldata_as_interaction() {
        // Zero-value no-input calls still count as an interaction
        // (self-sends, system calls on some L2s) because they do not
        // carry user-visible value.
        assert_eq!(
            TxCategory::classify(Some(sample_to()), &[], Wei::new(0)),
            TxCategory::Interaction,
        );
    }

    #[test]
    fn classifies_calldata_as_interaction() {
        assert_eq!(
            TxCategory::classify(Some(sample_to()), &[0xa9, 0x05, 0x9c, 0xbb], Wei::new(0)),
            TxCategory::Interaction,
        );
    }

    #[test]
    fn classifies_calldata_with_value_as_interaction() {
        // Even if value > 0, the presence of calldata means the
        // receiver contract decided what to do with the funds; this
        // is not a plain transfer.
        assert_eq!(
            TxCategory::classify(
                Some(sample_to()),
                &[0xa9, 0x05, 0x9c, 0xbb],
                Wei::new(1_000)
            ),
            TxCategory::Interaction,
        );
    }

    #[test]
    fn badge_and_label_are_stable() {
        assert_eq!(TxCategory::Transfer.badge(), "T");
        assert_eq!(TxCategory::Interaction.badge(), "I");
        assert_eq!(TxCategory::Deploy.badge(), "D");
        assert_eq!(TxCategory::Transfer.label(), "transfer");
        assert_eq!(TxCategory::Interaction.label(), "interaction");
        assert_eq!(TxCategory::Deploy.label(), "deploy");
    }
}
