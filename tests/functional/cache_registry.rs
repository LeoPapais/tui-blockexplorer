//! Unit tests for the cache-TTL registry.
//!
//! See `plan/15-backlog.md` §8.16 "Cache registry" — the registry is
//! the single source of truth for the TTL of every logical cache
//! namespace. These tests lock the advertised values in place so a
//! silent bump to a TTL constant cannot land without touching the
//! table in the doc comment (which humans actually review).

use std::time::Duration;

use blockexplorer_tui::adapters::{
    cache::{ABI_TTL, CacheRegistry, ENS_REVERSE_TTL, HEALTH_TTL, SEARCH_TTL},
    ens::DEFAULT_REVERSE_TTL,
    etherscan::DEFAULT_HINT_TTL,
};
use pretty_assertions::assert_eq;

#[test]
fn registry_defaults_match_documented_ttls() {
    let reg = CacheRegistry::default();
    assert_eq!(reg.ens_reverse, Duration::from_secs(300));
    assert_eq!(reg.abi, Duration::from_secs(300));
    assert_eq!(reg.search, Duration::from_secs(60));
    assert_eq!(reg.health, Duration::from_secs(30));
}

#[test]
fn production_registry_matches_the_default() {
    assert_eq!(CacheRegistry::production().search, Duration::from_secs(60));
    assert_eq!(
        CacheRegistry::production().ens_reverse,
        Duration::from_secs(300)
    );
}

#[test]
fn decorator_constants_are_wired_to_the_registry() {
    // The ENS reverse decorator and the Etherscan proxy-hint
    // decorator must read the same TTL as the registry; otherwise
    // swapping the constant in `registry.rs` would leave stale
    // hard-coded values in the adapters.
    assert_eq!(DEFAULT_REVERSE_TTL, ENS_REVERSE_TTL);
    assert_eq!(DEFAULT_HINT_TTL, ABI_TTL);
}

#[test]
fn registry_constants_are_consistent_with_rules() {
    // `.cursor/rules/external-apis.mdc` pins the ENS TTL to 5 minutes
    // and the search TTL to 60 seconds. Keep this test in sync with
    // the rule so reviewers see both files move together.
    assert_eq!(ENS_REVERSE_TTL, Duration::from_secs(300));
    assert_eq!(SEARCH_TTL, Duration::from_secs(60));
    assert_eq!(HEALTH_TTL, Duration::from_secs(30));
}
