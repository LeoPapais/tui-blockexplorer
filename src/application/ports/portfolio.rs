//! Outbound port returning the non-zero ERC-20 holdings of an address.
//!
//! Live adapter combines `alchemy_getTokenBalances` with a fan-out of
//! `alchemy_getTokenMetadata` calls (one per non-zero holding, capped
//! at a sensible top-N so we do not make hundreds of metadata calls
//! per wallet).
//!
//! See `plan/6-address-detail.md` section 12.4.2.

use crate::domain::{Address, Chain, DomainError, TokenHolding};

pub trait PortfolioPort: Send + Sync {
    fn get_token_balances(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Vec<TokenHolding>, DomainError>> + Send;
}
