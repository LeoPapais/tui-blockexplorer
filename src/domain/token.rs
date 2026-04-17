//! Fungible-token metadata and the overview used by the Token Detail
//! screen.
//!
//! See `plan/8-token-detail.md` section 12.1.

use crate::domain::Address;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenMetadata {
    pub address: Address,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
}

/// Overview shown on the Token Detail screen. Raw supply is stored
/// without unit scaling so callers can choose how to render it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenOverview {
    pub metadata: TokenMetadata,
    pub total_supply: u128,
}
