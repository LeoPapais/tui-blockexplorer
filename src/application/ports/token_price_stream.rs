//! Outbound port that feeds the Token Detail screen with a live
//! sequence of spot-price samples.
//!
//! Semantics mirror [`NewHeadsStreamPort`](super::NewHeadsStreamPort):
//! the receiver produces one [`PriceLookup`] per sample until the
//! upstream connection goes away, at which point the sender half is
//! dropped. Adapters may poll the Alchemy Prices API on a timer or
//! wrap a real streaming endpoint; both shapes expose the same trait.
//!
//! See `plan/8-token-detail.md` §13.1 and `plan/15-backlog.md` §8.9.

use tokio::sync::mpsc::UnboundedReceiver;

use crate::domain::{Address, Chain, DomainError, PriceLookup};

pub trait TokenPriceStreamPort: Send + Sync {
    /// Open a live price subscription for the given token on `chain`.
    ///
    /// Each received value represents one poll / push sample. The
    /// first sample may arrive immediately (adapters commonly emit
    /// an initial "warm-up" value before the first interval tick).
    /// Dropping the receiver tears down the upstream subscription.
    fn subscribe(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<UnboundedReceiver<PriceLookup>, DomainError>> + Send;
}
