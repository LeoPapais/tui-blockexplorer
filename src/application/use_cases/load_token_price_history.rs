//! Use case: fetch the historical price series for one token.
//!
//! See `plan/8-token-detail.md` section 4.3.

use crate::{
    application::ports::PricesPort,
    domain::{Address, Chain, DomainError, PriceSeries, PriceWindow},
};

/// Delegate to the Prices adapter. An empty series is a valid
/// outcome (no data in the requested window) and propagates as
/// `Ok(PriceSeries { points: vec![], ... })`.
pub async fn run<P: PricesPort>(
    prices: &P,
    address: Address,
    chain: Chain,
    window: PriceWindow,
) -> Result<PriceSeries, DomainError> {
    prices.get_history(address, chain, window).await
}
