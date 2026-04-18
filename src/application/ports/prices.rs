//! Outbound port for spot + historical token prices.
//!
//! Backed by the Alchemy Prices API
//! (`https://api.g.alchemy.com/prices/v1/...`) in live mode and by an
//! in-memory stub in tests.
//!
//! See `plan/8-token-detail.md` section 12.4 and
//! `plan/15-backlog.md` §3.4 for the `PriceLookup` status type.

use crate::domain::{Address, Chain, DomainError, PriceLookup, PriceSeries, PriceWindow};

pub trait PricesPort: Send + Sync {
    /// Latest spot price for one token. The returned
    /// [`PriceLookup`] carries the three states surfaced by the
    /// live Alchemy Prices API:
    ///
    /// - `Available(price)` on a successful hit.
    /// - `Unsupported { provider }` when the provider declines to
    ///   answer (404 / empty response).
    /// - `Pending` is never returned by the port itself; it is the
    ///   default state callers (UI / use cases) use before the port
    ///   resolves.
    fn get_single(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<PriceLookup, DomainError>> + Send;

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
