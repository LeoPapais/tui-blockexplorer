//! Functional tests for the TTL-cache wiring on the search feed.
//!
//! See `plan/2-search.md` section 12.4.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::{cache::TtlCache, ui::search_feed},
    domain::{Chain, ResolvedEntity, TxHash, TxSummary},
    infra::search_feed as feed_task,
    infra::search_feed::SearchCache,
};
use pretty_assertions::assert_eq;
use tokio::time::{sleep, timeout};

use crate::support::stubs::{
    FrozenClock, StubAddressLookupPort, StubBlockLookupPort, StubEnsResolverPort,
    StubTokenReaderPort, StubTokenSearchPort, StubTxLookupPort,
};

const TX_HEX: &str = "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b";

fn known_tx() -> TxHash {
    TxHash::from_hex(TX_HEX).unwrap()
}

fn primed_tx_stub() -> StubTxLookupPort {
    let tx = StubTxLookupPort::new();
    tx.insert(TxSummary {
        hash: known_tx(),
        block: None,
    });
    tx
}

async fn drive_once(
    input: &str,
    tx_stub: StubTxLookupPort,
    cache: Option<SearchCache<FrozenClock>>,
) -> (StubTxLookupPort, Vec<Vec<ResolvedEntity>>) {
    let block = StubBlockLookupPort::new();
    let address = StubAddressLookupPort::new();
    let ens = StubEnsResolverPort::new();
    let token = StubTokenSearchPort::new();
    let token_reader = StubTokenReaderPort::new();

    let (feed, sender) = search_feed();
    let handle = feed_task::spawn_with_cache(
        Chain::Ethereum,
        block,
        tx_stub.clone(),
        address,
        ens,
        token,
        token_reader,
        sender,
        cache,
    );

    feed.input_tx.send(input.to_string()).unwrap();

    let mut rx = feed.updates_rx;
    let mut updates: Vec<Vec<ResolvedEntity>> = Vec::new();
    while let Ok(Some(update)) = timeout(Duration::from_millis(300), rx.recv()).await {
        updates.push(update.candidates);
    }
    drop(feed.input_tx);
    let _ = handle.await;

    (tx_stub, updates)
}

#[tokio::test]
async fn repeated_search_within_ttl_hits_the_cache_and_skips_lookups() {
    let clock = FrozenClock::default();
    let cache: SearchCache<FrozenClock> =
        TtlCache::with_ttl_and_clock(Duration::from_secs(60), clock.clone());

    let tx = primed_tx_stub();
    let (tx, first) = drive_once(TX_HEX, tx, Some(cache.clone())).await;
    let count_after_first = tx.call_count();
    assert!(
        count_after_first >= 1,
        "first search must exercise the tx lookup",
    );
    assert!(
        matches!(first.last(), Some(v) if matches!(v.first(), Some(ResolvedEntity::Tx { .. }))),
        "first search must surface a Tx row",
    );

    // Stay well within the 60 s TTL.
    clock.advance(Duration::from_secs(30));

    let (tx_after, second) = drive_once(TX_HEX, tx, Some(cache.clone())).await;
    assert_eq!(
        tx_after.call_count(),
        count_after_first,
        "repeated search within TTL must not hit the tx stub again",
    );
    assert!(
        matches!(second.last(), Some(v) if matches!(v.first(), Some(ResolvedEntity::Tx { .. }))),
        "cached response keeps the Tx row",
    );
}

#[tokio::test]
async fn search_past_ttl_re_runs_the_underlying_lookup() {
    let clock = FrozenClock::default();
    let cache: SearchCache<FrozenClock> =
        TtlCache::with_ttl_and_clock(Duration::from_secs(60), clock.clone());

    let tx = primed_tx_stub();
    let (tx, _) = drive_once(TX_HEX, tx, Some(cache.clone())).await;
    let first_count = tx.call_count();

    clock.advance(Duration::from_secs(61));

    let (tx_after, _) = drive_once(TX_HEX, tx, Some(cache)).await;
    assert!(
        tx_after.call_count() > first_count,
        "past the TTL the resolver runs again",
    );
}

#[tokio::test]
async fn not_found_results_are_not_cached() {
    let clock = FrozenClock::default();
    let cache: SearchCache<FrozenClock> =
        TtlCache::with_ttl_and_clock(Duration::from_secs(60), clock.clone());

    // Tx stub with no entry — `query.run` yields a NotFound row.
    let tx = StubTxLookupPort::new();
    let (tx, first) = drive_once(TX_HEX, tx, Some(cache.clone())).await;
    assert!(matches!(
        first.last().and_then(|v| v.first()),
        Some(ResolvedEntity::NotFound { .. })
    ));
    let count_after_first = tx.call_count();

    // The cache must be untouched, so a repeat still dispatches the
    // lookup (no NotFound poisoning for the full TTL).
    clock.advance(Duration::from_secs(5));
    let (tx_after, _) = drive_once(TX_HEX, tx, Some(cache)).await;
    assert!(
        tx_after.call_count() > count_after_first,
        "NotFound must not be cached",
    );
    // Give any stray tasks time to quiesce before dropping the
    // runtime — makes the test robust under Miri-less CI.
    sleep(Duration::from_millis(10)).await;
}

#[tokio::test]
async fn normalised_input_shares_a_cache_slot_across_quote_and_case_variants() {
    let clock = FrozenClock::default();
    let cache: SearchCache<FrozenClock> =
        TtlCache::with_ttl_and_clock(Duration::from_secs(60), clock.clone());

    let tx = primed_tx_stub();
    let (tx, _) = drive_once(TX_HEX, tx, Some(cache.clone())).await;
    let count_after_first = tx.call_count();

    let upper_quoted = format!(" '{}' ", TX_HEX.to_uppercase());
    let (tx_after, _) = drive_once(&upper_quoted, tx, Some(cache)).await;
    assert_eq!(
        tx_after.call_count(),
        count_after_first,
        "uppercase-hex + surrounding quotes must hit the same cache slot",
    );
}
