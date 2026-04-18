//! Outbound port for spot + historical token prices.
//!
//! Backed by the Alchemy Prices API
//! (`https://api.g.alchemy.com/prices/v1/...`) in live mode and by an
//! in-memory stub in tests.
//!
//! See `plan/8-token-detail.md` section 12.4.

use crate::domain::{Address, Chain, DomainError, PriceSeries, PriceWindow, TokenPrice};

pub trait PricesPort: Send + Sync {
    /// Latest spot price for one token. Returns `Ok(None)` when the
    /// provider has no data for this address.
    fn get_single(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<TokenPrice>, DomainError>> + Send;

    /// Historical series covering the selected `window`. Points are
    /// returned in chronological (ascending) order. An empty series
    /// is a legitimate response and must not be mapped to an error.
    fn get_history(
        &self,
        address: Address,
        chain: Chain,
        window: PriceWindow,
    ) -> impl std::future::Future<Output = Result<PriceSeries, DomainError>> + Send;
}
