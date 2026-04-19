//! TTL cache decorator for [`EtherscanProxyHintPort`].
//!
//! Mirrors [`crate::adapters::ens::cached::CachedEnsResolver`]. Five-
//! minute TTL per `.cursor/rules/external-apis.mdc` and plan/7 §12.5.1.
//! `None` results are cached so that a non-proxy contract does not
//! hammer Etherscan every time the user reopens Contract Detail.

use std::time::Duration;

use crate::{
    adapters::{
        cache::{ABI_TTL, TtlCache},
        clock::SystemClock,
    },
    application::ports::{Clock, EtherscanProxyHintPort},
    domain::{Address, Chain, DomainError},
};

/// Default TTL for the proxy-hint cache. Pulled from the shared cache
/// registry (`adapters::cache::ABI_TTL`) so the ABI / proxy-hint
/// family of caches always agrees on a single 5-minute budget.
pub const DEFAULT_HINT_TTL: Duration = ABI_TTL;

/// Decorator that wraps any [`EtherscanProxyHintPort`] with a
/// TTL-bounded cache keyed by `(chain, address)`.
#[derive(Clone)]
pub struct CachedEtherscanProxyHint<H, C: Clock + 'static = SystemClock> {
    inner: H,
    cache: TtlCache<(Chain, Address), Option<Address>, C>,
}

impl<H> CachedEtherscanProxyHint<H, SystemClock>
where
    H: EtherscanProxyHintPort,
{
    #[must_use]
    pub fn new(inner: H) -> Self {
        Self::with_ttl(inner, DEFAULT_HINT_TTL)
    }

    #[must_use]
    pub fn with_ttl(inner: H, ttl: Duration) -> Self {
        Self::with_ttl_and_clock(inner, ttl, SystemClock::new())
    }
}

impl<H, C> CachedEtherscanProxyHint<H, C>
where
    H: EtherscanProxyHintPort,
    C: Clock + 'static,
{
    #[must_use]
    pub fn with_ttl_and_clock(inner: H, ttl: Duration, clock: C) -> Self {
        Self {
            inner,
            cache: TtlCache::with_ttl_and_clock(ttl, clock),
        }
    }
}

impl<H, C> EtherscanProxyHintPort for CachedEtherscanProxyHint<H, C>
where
    H: EtherscanProxyHintPort,
    C: Clock + Clone + 'static,
{
    async fn implementation_hint(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<Option<Address>, DomainError> {
        let key = (chain, address);
        if let Some(cached) = self.cache.get(&key).await {
            return Ok(cached);
        }
        let fresh = self.inner.implementation_hint(address, chain).await?;
        self.cache.insert(key, fresh).await;
        Ok(fresh)
    }
}
