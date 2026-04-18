//! Functional tests for `TtlCache`.
//!
//! Covers the happy path (hit inside TTL), expiry past TTL driven by
//! `FrozenClock`, and concurrent reads through `Arc<TtlCache>`.
//!
//! See `plan/2-search.md` section 12.3.

use std::{sync::Arc, time::Duration};

use blockexplorer_tui::adapters::cache::memory::TtlCache;
use pretty_assertions::assert_eq;

use crate::support::stubs::FrozenClock;

fn cache(
    ttl_secs: u64,
    clock: FrozenClock,
) -> TtlCache<(&'static str, u64), String, FrozenClock> {
    TtlCache::with_ttl_and_clock(Duration::from_secs(ttl_secs), clock)
}

#[tokio::test]
async fn hit_within_ttl_returns_cached_value() {
    let clock = FrozenClock::default();
    let c = cache(60, clock.clone());
    c.insert(("ethereum", 1), "one".to_string()).await;

    clock.advance(Duration::from_secs(30));

    assert_eq!(c.get(&("ethereum", 1)).await.as_deref(), Some("one"));
}

#[tokio::test]
async fn expired_entry_is_dropped_and_reports_miss() {
    let clock = FrozenClock::default();
    let c = cache(60, clock.clone());
    c.insert(("ethereum", 1), "one".to_string()).await;

    clock.advance(Duration::from_secs(61));

    assert!(c.get(&("ethereum", 1)).await.is_none());
}

#[tokio::test]
async fn insert_overwrites_previous_value_and_resets_ttl() {
    let clock = FrozenClock::default();
    let c = cache(60, clock.clone());
    c.insert(("ethereum", 1), "one".to_string()).await;
    clock.advance(Duration::from_secs(30));
    c.insert(("ethereum", 1), "two".to_string()).await;

    // Past the original TTL but within the refreshed one.
    clock.advance(Duration::from_secs(45));

    assert_eq!(c.get(&("ethereum", 1)).await.as_deref(), Some("two"));
}

#[tokio::test]
async fn concurrent_gets_do_not_deadlock() {
    let clock = FrozenClock::default();
    let c = Arc::new(cache(60, clock));
    c.insert(("ethereum", 1), "one".to_string()).await;

    let mut handles = Vec::new();
    for _ in 0..16 {
        let c = Arc::clone(&c);
        handles.push(tokio::spawn(async move { c.get(&("ethereum", 1)).await }));
    }

    for h in handles {
        assert_eq!(h.await.unwrap().as_deref(), Some("one"));
    }
}

#[tokio::test]
async fn miss_on_unknown_key() {
    let clock = FrozenClock::default();
    let c = cache(60, clock);
    assert!(c.get(&("ethereum", 999)).await.is_none());
}
