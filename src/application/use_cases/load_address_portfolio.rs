//! Use case: load the non-zero ERC-20 portfolio of an address.
//!
//! See `plan/6-address-detail.md` section 12.4.2 and
//! `plan/15-backlog.md` §3.4 for the per-holding `PriceLookup`
//! fan-out.

use crate::{
    application::ports::{PortfolioPort, PricesPort},
    domain::{Address, Chain, DomainError, TokenHolding},
};

/// Fetch the wallet's holdings and tag each one with a
/// [`PriceLookup`] by calling the Prices port per contract. The
/// portfolio itself is returned unchanged when the Prices port
/// errors for a specific entry — a single failed spot price must not
/// drop a holding from the list.
///
/// [`PriceLookup`]: crate::domain::PriceLookup
pub async fn run<P, R>(
    port: &P,
    prices: &R,
    address: Address,
    chain: Chain,
) -> Result<Vec<TokenHolding>, DomainError>
where
    P: PortfolioPort,
    R: PricesPort,
{
    let mut holdings = port.get_token_balances(address, chain).await?;
    for holding in &mut holdings {
        // A broken Prices call for one token keeps `price` as the
        // reader's default (`PriceLookup::Pending`); the UI falls
        // back to "loading…" for that row instead of dropping the
        // entire holding. Any other row still gets a real
        // Available / Unsupported value.
        if let Ok(lookup) = prices.get_single(holding.metadata.address, chain).await {
            holding.price = lookup;
        }
    }
    Ok(holdings)
}
