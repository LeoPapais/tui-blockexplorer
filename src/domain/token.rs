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

impl TokenMetadata {
    /// Whether this metadata is incomplete enough that the Token
    /// Detail screen should fall back to the "not a standard ERC-20"
    /// empty state.
    ///
    /// Two degenerate shapes we've seen coming back from
    /// `alchemy_getTokenMetadata` on non-ERC-20 contracts:
    ///
    /// - `symbol` is empty: no ticker at all.
    /// - `name` and `decimals` are both zero-valued: most token
    ///   contracts expose at least one of the two, so losing both is
    ///   a strong signal the address is not a standard ERC-20.
    ///
    /// See `plan/8-token-detail.md` §13.2 and
    /// `plan/15-backlog.md` §8.9.
    #[must_use]
    pub fn is_incomplete(&self) -> bool {
        self.symbol.is_empty() || (self.decimals == 0 && self.name.is_empty())
    }
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
    /// Delegates to [`TokenMetadata::is_incomplete`]. When this
    /// returns `true` the Token Detail screen swaps the Overview tab
    /// body for the "non-standard ERC-20" empty state.
    #[must_use]
    pub fn is_incomplete(&self) -> bool {
        self.metadata.is_incomplete()
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn addr() -> Address {
        Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
    }

    #[test]
    fn is_incomplete_flags_empty_symbol() {
        let md = TokenMetadata {
            address: addr(),
            symbol: String::new(),
            name: "Some Token".into(),
            decimals: 18,
        };
        assert!(md.is_incomplete());
    }

    #[test]
    fn is_incomplete_flags_missing_decimals_and_name() {
        let md = TokenMetadata {
            address: addr(),
            symbol: "XYZ".into(),
            name: String::new(),
            decimals: 0,
        };
        assert!(md.is_incomplete());
    }

    #[test]
    fn is_incomplete_is_false_for_a_standard_erc20() {
        let md = TokenMetadata {
            address: addr(),
            symbol: "USDC".into(),
            name: "USD Coin".into(),
            decimals: 6,
        };
        assert!(!md.is_incomplete());
    }

    #[test]
    fn token_overview_is_incomplete_delegates() {
        let md = TokenMetadata {
            address: addr(),
            symbol: String::new(),
            name: String::new(),
            decimals: 0,
        };
        let ov = TokenOverview {
            metadata: md,
            total_supply: 0,
            price: PriceLookup::Pending,
        };
        assert!(ov.is_incomplete());
    }
}
