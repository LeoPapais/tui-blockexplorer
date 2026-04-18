//! Fungible-token metadata and the overview used by the Token Detail
//! screen.
//!
//! See `plan/8-token-detail.md` section 12.1.

use crate::domain::{Address, TokenPrice};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenMetadata {
    pub address: Address,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
}

/// Overview shown on the Token Detail screen. Raw supply is stored
/// without unit scaling so callers can choose how to render it.
///
/// `price` is `None` when the Prices API returns no data or when
/// it is not wired (demo mode, missing credentials).
#[derive(Debug, Clone, PartialEq)]
pub struct TokenOverview {
    pub metadata: TokenMetadata,
    pub total_supply: u128,
    pub price: Option<TokenPrice>,
}

impl TokenOverview {
    /// Market cap derived from `price × totalSupply / 10^decimals`.
    /// Returns `None` when price is missing or `totalSupply` is zero
    /// (no meaningful cap).
    #[must_use]
    pub fn market_cap(&self) -> Option<f64> {
        let price = self.price.as_ref()?;
        if self.total_supply == 0 {
            return None;
        }
        let supply_human =
            (self.total_supply as f64) / 10f64.powi(i32::from(self.metadata.decimals));
        Some(supply_human * price.value)
    }
}
