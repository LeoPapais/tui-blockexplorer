//! Token portfolio for an address.
//!
//! Feeds the Tokens tab on the Address Detail screen. USD pricing
//! stays deferred; the MVP only tracks symbol / decimals / raw
//! balance. See `plan/6-address-detail.md` section 12.4.2.

use crate::domain::{TokenMetadata, Wei};

/// Single non-zero ERC-20 holding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenHolding {
    pub metadata: TokenMetadata,
    /// Raw balance in the token's smallest denomination.
    pub balance: Wei,
}
