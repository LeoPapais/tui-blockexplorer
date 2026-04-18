//! Functional tests for [`CachedEnsResolver`].
//!
//! Exercises the 5-minute TTL rule from
//! `.cursor/rules/external-apis.mdc` plus `plan/6-address-detail.md`
//! §11 "Shipped". The cache wraps any [`EnsResolverPort`] and keys
//! by `(chain, address)`; the forward path remains uncached (the
//! search feed already owns a 60 s cache for the enriched
//! candidate list).

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ens::CachedEnsResolver, application::ports::EnsResolverPort,
    domain::{Address, Chain},
};

use crate::support::stubs::{FrozenClock, StubEnsResolverPort};

fn addr(hex: &str) -> Address {
    Address::from_hex(hex).unwrap()
}

#[tokio::test]
async fn reverse_caches_a_hit_within_the_ttl() {
    let inner = StubEnsResolverPort::new();
    let vitalik = addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
    inner.set_reverse(vitalik, "vitalik.eth");
    let clock = FrozenClock::default();
    let cached =
        CachedEnsResolver::with_ttl_and_clock(inner.clone(), Duration::from_secs(300), clock.clone());

    let first = cached.reverse(vitalik, Chain::Ethereum).await.unwrap();
    assert_eq!(first.as_deref(), Some("vitalik.eth"));

    // Drop the inner mapping: a second lookup that hits the cache
    // must return the same value even though the backing stub no
    // longer has the entry.
    inner.clear_reverse();
    clock.advance(Duration::from_secs(60));
    let second = cached.reverse(vitalik, Chain::Ethereum).await.unwrap();
    assert_eq!(second.as_deref(), Some("vitalik.eth"));
}

#[tokio::test]
async fn reverse_refreshes_after_ttl_expires() {
    let inner = StubEnsResolverPort::new();
    let vitalik = addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
    inner.set_reverse(vitalik, "vitalik.eth");
    let clock = FrozenClock::default();
    let cached =
        CachedEnsResolver::with_ttl_and_clock(inner.clone(), Duration::from_secs(300), clock.clone());

    let _ = cached.reverse(vitalik, Chain::Ethereum).await.unwrap();

    inner.clear_reverse();
    clock.advance(Duration::from_secs(301));
    let after = cached.reverse(vitalik, Chain::Ethereum).await.unwrap();
    assert!(after.is_none(), "expired entry must fall through to inner");
}

#[tokio::test]
async fn reverse_caches_a_miss() {
    let inner = StubEnsResolverPort::new();
    let rogue = addr("0x0000000000000000000000000000000000000099");
    let clock = FrozenClock::default();
    let cached =
        CachedEnsResolver::with_ttl_and_clock(inner.clone(), Duration::from_secs(300), clock.clone());

    let first = cached.reverse(rogue, Chain::Ethereum).await.unwrap();
    assert!(first.is_none());

    inner.set_reverse(rogue, "late-registered.eth");
    let second = cached.reverse(rogue, Chain::Ethereum).await.unwrap();
    assert!(
        second.is_none(),
        "cached miss must not be overwritten before expiry",
    );

    clock.advance(Duration::from_secs(301));
    let third = cached.reverse(rogue, Chain::Ethereum).await.unwrap();
    assert_eq!(third.as_deref(), Some("late-registered.eth"));
}

#[tokio::test]
async fn forward_is_not_cached() {
    let inner = StubEnsResolverPort::new();
    let name = "vitalik.eth";
    let vitalik = addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
    inner.set_forward(name, vitalik);
    let clock = FrozenClock::default();
    let cached =
        CachedEnsResolver::with_ttl_and_clock(inner.clone(), Duration::from_secs(300), clock.clone());

    let first = cached.forward(name, Chain::Ethereum).await.unwrap();
    assert_eq!(first, Some(vitalik));

    // Clearing the inner mapping must cause the next forward call
    // to return `None` — the cache explicitly skips forward
    // resolution because the search feed already holds its own
    // 60 s cache (plan/2 §12.4).
    inner.clear_forward();
    let second = cached.forward(name, Chain::Ethereum).await.unwrap();
    assert!(second.is_none());
}
