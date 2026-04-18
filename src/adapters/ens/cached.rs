//! TTL cache decorator for [`EnsResolverPort`] reverse lookups.
//!
//! `.cursor/rules/external-apis.mdc` prescribes a 5-minute TTL for
//! ENS results. The Search feed already owns a 60-second cache for
//! the enriched candidate list, so this decorator *only* caches the
//! reverse path (the flow triggered by Address Detail and by the
//! Search result enrichment). Forward resolution stays uncached so
//! a freshly-registered name resolves immediately; re-running
//! forward resolution is also cheap (two `eth_call`s) and gated by
//! the upstream 60 s cache.
//!
//! See `plan/6-address-detail.md` §11 "Shipped" and
//! `plan/15-backlog.md` §8.7.

use std::time::Duration;

use crate::{
    adapters::{cache::TtlCache, clock::SystemClock},
    application::ports::{Clock, EnsResolverPort},
    domain::{Address, Chain, DomainError},
};

/// Five-minute TTL for the reverse cache. Encoded here so the
/// production composition root can pick it up directly.
pub const DEFAULT_REVERSE_TTL: Duration = Duration::from_secs(300);

/// Decorator that wraps any [`EnsResolverPort`] with a TTL-bounded
/// cache for the reverse lookup. `None` results are cached too so
/// that lookups for wallets without a reverse record do not
/// hammer the RPC every time the user opens Address Detail.
#[derive(Clone)]
pub struct CachedEnsResolver<E, C: Clock + 'static = SystemClock> {
    inner: E,
    reverse: TtlCache<(Chain, Address), Option<String>, C>,
}

impl<E> CachedEnsResolver<E, SystemClock>
where
    E: EnsResolverPort,
{
    /// Construct a cache backed by the wall clock with the default
    /// 5-minute TTL.
    #[must_use]
    pub fn new(inner: E) -> Self {
        Self::with_ttl(inner, DEFAULT_REVERSE_TTL)
    }

    /// Construct a cache with a custom TTL.
    #[must_use]
    pub fn with_ttl(inner: E, ttl: Duration) -> Self {
        Self::with_ttl_and_clock(inner, ttl, SystemClock::new())
    }
}

impl<E, C> CachedEnsResolver<E, C>
where
    E: EnsResolverPort,
    C: Clock + 'static,
{
    /// Construct a cache with a custom TTL and clock. Intended for
    /// tests that drive expiry through a `FrozenClock`.
    #[must_use]
    pub fn with_ttl_and_clock(inner: E, ttl: Duration, clock: C) -> Self {
        Self {
            inner,
            reverse: TtlCache::with_ttl_and_clock(ttl, clock),
        }
    }
}

impl<E, C> EnsResolverPort for CachedEnsResolver<E, C>
where
    E: EnsResolverPort,
    C: Clock + Clone + 'static,
{
    async fn forward(&self, name: &str, chain: Chain) -> Result<Option<Address>, DomainError> {
        // Forward resolution is not cached at this layer — see the
        // module doc-comment and `plan/2 §12.4` for the higher-level
        // search cache.
        self.inner.forward(name, chain).await
    }

    async fn reverse(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<Option<String>, DomainError> {
        let key = (chain, address);
        if let Some(cached) = self.reverse.get(&key).await {
            return Ok(cached);
        }
        let fresh = self.inner.reverse(address, chain).await?;
        self.reverse.insert(key, fresh.clone()).await;
        Ok(fresh)
    }
}
