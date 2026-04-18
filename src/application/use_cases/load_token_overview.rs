//! Use case: load the Token Detail Overview view-model.
//!
//! See `plan/8-token-detail.md` section 12.1 and
//! `plan/15-backlog.md` §3.4 for the per-token price status.

use crate::{
    application::ports::{PricesPort, TokenReaderPort},
    domain::{Address, Chain, DomainError, PriceLookup, TokenOverview},
};

/// Fetch the Token Overview view-model (metadata + totalSupply) and
/// attach the `PriceLookup` status returned by the Prices port. The
/// reader is authoritative for metadata and supply; the price field
/// on the returned `TokenOverview` is always overwritten with the
/// Prices port answer so downstream consumers never see a stale
/// `Pending` once this use case has run.
pub async fn run<R, P>(
    reader: &R,
    prices: &P,
    address: Address,
    chain: Chain,
) -> Result<TokenOverview, DomainError>
where
    R: TokenReaderPort,
    P: PricesPort,
{
    match reader.get(address, chain).await? {
        Some(mut overview) => {
            overview.price = prices.get_single(address, chain).await?;
            Ok(overview)
        }
        None => Err(DomainError::NotFound),
    }
}

/// Variant kept for the screen wiring that fetches the price on a
/// separate channel. Returns the reader's overview untouched, with
/// `price` left as `PriceLookup::Pending` so the UI can flip it to
/// `Available` / `Unsupported` the moment the price feed resolves.
///
/// TODO(plan/15-backlog.md §3.4): collapse into `run` once all
/// callers have migrated off the split-channel layout.
pub async fn run_metadata_only<R>(
    reader: &R,
    address: Address,
    chain: Chain,
) -> Result<TokenOverview, DomainError>
where
    R: TokenReaderPort,
{
    match reader.get(address, chain).await? {
        Some(mut overview) => {
            overview.price = PriceLookup::Pending;
            Ok(overview)
        }
        None => Err(DomainError::NotFound),
    }
}
