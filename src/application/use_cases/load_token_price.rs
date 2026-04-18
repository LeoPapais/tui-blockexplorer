//! Use case: fetch the spot price for a token.
//!
//! See `plan/8-token-detail.md` section 4.4 and
//! `plan/15-backlog.md` §3.4.

use crate::{
    application::ports::PricesPort,
    domain::{Address, Chain, DomainError, PriceLookup},
};

/// Returns a [`PriceLookup`]: `Available` on a hit, `Unsupported`
/// when the provider declines (404 / empty payload), or a bubbled-up
/// provider error. `PriceLookup::Pending` is never emitted by this
/// use case; callers hold that state themselves while the future is
/// in flight.
pub async fn run<P: PricesPort>(
    prices: &P,
    address: Address,
    chain: Chain,
) -> Result<PriceLookup, DomainError> {
    prices.get_single(address, chain).await
}
