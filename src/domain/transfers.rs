//! Unified transfers feed for an address.
//!
//! Mirrors Alchemy's `alchemy_getAssetTransfers` in a
//! provider-agnostic shape: the Transactions tab in the Address
//! Detail screen consumes a `TransferPage`, and the future Activity
//! tab will classify the same events into higher-level user stories.
//!
//! See `plan/6-address-detail.md` section 12.4.1.

use crate::domain::{Address, BlockNumber, Chain, TxHash, Wei};

/// Logical category of a transfer, matching Alchemy's taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferCategory {
    External,
    Internal,
    Erc20,
    Erc721,
    Erc1155,
}

impl TransferCategory {
    #[must_use]
    pub const fn as_alchemy_param(self) -> &'static str {
        match self {
            TransferCategory::External => "external",
            TransferCategory::Internal => "internal",
            TransferCategory::Erc20 => "erc20",
            TransferCategory::Erc721 => "erc721",
            TransferCategory::Erc1155 => "erc1155",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            TransferCategory::External => "EXTERNAL",
            TransferCategory::Internal => "INTERNAL",
            TransferCategory::Erc20 => "ERC20",
            TransferCategory::Erc721 => "ERC721",
            TransferCategory::Erc1155 => "ERC1155",
        }
    }
}

/// NFT flavour surfaced inside [`TransferAsset::Nft`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NftKind {
    Erc721,
    Erc1155,
}

/// Asset moved in a transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferAsset {
    /// Chain-native value transfer (ETH / MATIC / ...).
    Native { symbol: String },
    /// ERC-20 fungible transfer.
    Erc20 {
        contract: Address,
        symbol: String,
        decimals: u8,
    },
    /// ERC-721 or ERC-1155 NFT transfer. `value` on the parent
    /// `TransferEvent` carries the amount (usually 1 for 721).
    Nft {
        contract: Address,
        kind: NftKind,
        token_id: String,
        symbol: String,
    },
}

impl TransferAsset {
    #[must_use]
    pub fn symbol(&self) -> &str {
        match self {
            TransferAsset::Native { symbol }
            | TransferAsset::Erc20 { symbol, .. }
            | TransferAsset::Nft { symbol, .. } => symbol,
        }
    }
}

/// Single transfer event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferEvent {
    pub chain: Chain,
    pub block_number: BlockNumber,
    pub tx_hash: TxHash,
    pub from: Address,
    /// `None` for contract-creation transfers.
    pub to: Option<Address>,
    pub asset: TransferAsset,
    /// Raw value in wei / smallest denomination; for NFTs this is
    /// usually 1.
    pub value: Wei,
    pub category: TransferCategory,
}

/// Opaque pagination cursor. Backs two Alchemy `pageKey`s (one per
/// direction) joined with `|`. Callers treat it as a black-box
/// string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferCursor(pub String);

/// One page of transfers returned by [`TransfersPort`]. Events are
/// pre-sorted by `block_number` descending and capped at a sensible
/// limit by the adapter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransferPage {
    pub events: Vec<TransferEvent>,
    pub next_cursor: Option<TransferCursor>,
}
