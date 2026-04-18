//! Token portfolio for an address.
//!
//! Feeds the Tokens tab on the Address Detail screen. See
//! `plan/6-address-detail.md` section 12.4.2 and
//! `plan/15-backlog.md` §3.4 for the per-holding `PriceLookup`.

use crate::domain::{PriceLookup, TokenMetadata, Wei};

/// Single non-zero ERC-20 holding.
///
/// `price` starts as [`PriceLookup::Pending`] right after the
/// portfolio fetch and is overwritten by `load_address_portfolio`
/// once the Prices API answers for the contract.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenHolding {
    pub metadata: TokenMetadata,
    /// Raw balance in the token's smallest denomination.
    pub balance: Wei,
    pub price: PriceLookup,
}
