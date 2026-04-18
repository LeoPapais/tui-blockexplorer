//! Use case: load the non-zero ERC-20 portfolio of an address.
//!
//! See `plan/6-address-detail.md` section 12.4.2.

use crate::{
    application::ports::PortfolioPort,
    domain::{Address, Chain, DomainError, TokenHolding},
};

pub async fn run<P: PortfolioPort>(
    port: &P,
    address: Address,
    chain: Chain,
) -> Result<Vec<TokenHolding>, DomainError> {
    port.get_token_balances(address, chain).await
}
