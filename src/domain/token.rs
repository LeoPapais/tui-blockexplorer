//! Fungible-token metadata and the overview used by the Token Detail
//! screen.
//!
//! See `plan/8-token-detail.md` section 12.1 and `plan/15-backlog.md`
//! §3.4 for the `PriceLookup` status used by the price field.

use crate::domain::{Address, PriceLookup};

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
/// `price` tracks the three states surfaced by the Prices API
/// adapter: a concrete spot price, an "unsupported" marker when
/// the provider declined to answer, or a pending lookup still in
/// flight. See `plan/15-backlog.md` §3.4.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenOverview {
    pub metadata: TokenMetadata,
    pub total_supply: u128,
    pub price: PriceLookup,
}

impl TokenOverview {
    /// Market cap derived from `price × totalSupply / 10^decimals`.
    /// Returns `None` when the price is not `Available` or
    /// `totalSupply` is zero (no meaningful cap).
    #[must_use]
    pub fn market_cap(&self) -> Option<f64> {
        let price = self.price.as_available()?;
        if self.total_supply == 0 {
            return None;
        }
        let supply_human =
            (self.total_supply as f64) / 10f64.powi(i32::from(self.metadata.decimals));
        Some(supply_human * price.value)
    }
}
