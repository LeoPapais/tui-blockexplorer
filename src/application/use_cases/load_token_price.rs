//! Use case: fetch the spot price for a token.
//!
//! See `plan/8-token-detail.md` section 4.4.

use crate::{
    application::ports::PricesPort,
    domain::{Address, Chain, DomainError, TokenPrice},
};

/// Returns `Ok(Some)` on a hit, `Ok(None)` when the provider has no
/// data for this address, or a bubbled-up provider error.
pub async fn run<P: PricesPort>(
    prices: &P,
    address: Address,
    chain: Chain,
) -> Result<Option<TokenPrice>, DomainError> {
    prices.get_single(address, chain).await
}
