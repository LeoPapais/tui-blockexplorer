//! Asset-change simulation output used by the TxDetail `Asset Changes`
//! tab. Shape mirrors Alchemy's `alchemy_simulateAssetChanges` API
//! with a thin domain wrapper so the UI stays provider-agnostic.
//!
//! See `plan/4-tx-detail.md` section 12.4.3.

use crate::domain::{Address, Wei};

/// Kind of asset that moved during a simulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetKind {
    /// Chain-native asset (ETH, MATIC, ...).
    Native,
    Erc20 {
        contract: Address,
        symbol: String,
        decimals: u8,
    },
    Erc721 {
        contract: Address,
        symbol: String,
        token_id: String,
    },
    Erc1155 {
        contract: Address,
        token_id: String,
        symbol: String,
    },
}

/// Kind of change. Other values from the API are folded into `Other`
/// so we do not crash on unseen variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetChangeKind {
    Transfer,
    Approve,
    Other,
}

/// Single entry in the asset-changes list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetChange {
    pub kind: AssetChangeKind,
    pub asset: AssetKind,
    /// Origin address. Some APIs return null when the change is a
    /// pure mint; that maps to `None`.
    pub from: Option<Address>,
    pub to: Option<Address>,
    /// Raw amount (wei / smallest denomination).
    pub amount: Wei,
}
