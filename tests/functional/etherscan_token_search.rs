//! Wiremock-driven tests for `EtherscanTokenSearch`.
//!
//! Covers: curated ticker lookup enriches the name via
//! `contract/getsourcecode`, ticker misses never hit the network,
//! free-text `by_name` matches curated entries without HTTP, and
//! Etherscan rate-limit errors degrade to the curated `name` without
//! bubbling the failure. See `plan/2-search.md` section 10.2.

use blockexplorer_tui::{
    adapters::etherscan::{EtherscanClient, EtherscanTokenSearch},
    application::ports::TokenSearchPort,
    domain::Chain,
};
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

use crate::support::fixture_loader::load_text;

fn adapter_for(url: &str) -> EtherscanTokenSearch {
    let client = EtherscanClient::new(
        Url::parse(&format!("{url}/v2/api")).unwrap(),
        "test-key".to_string(),
        reqwest::Client::new(),
    );
    EtherscanTokenSearch::new(client)
}

#[tokio::test]
async fn by_symbol_hit_enriches_name_from_getsourcecode() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("module", "contract"))
        .and(query_param("action", "getsourcecode"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__token_search__usdc.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let matches = adapter
        .by_symbol("USDC", Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(matches.len(), 1);
    let meta = &matches[0];
    assert_eq!(meta.symbol, "USDC");
    assert_eq!(meta.name, "FiatTokenV2_2");
    assert_eq!(meta.decimals, 6);
    assert_eq!(
        meta.address.to_hex(),
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
    );
}

#[tokio::test]
async fn by_symbol_miss_returns_empty_without_hitting_etherscan() {
    let server = MockServer::start().await;
    // Intentionally do NOT mount any routes. If the adapter calls
    // out, wiremock returns 404 and the test panics.
    let adapter = adapter_for(&server.uri());

    let matches = adapter
        .by_symbol("NOTATOKEN", Chain::Ethereum)
        .await
        .expect("ok");

    assert!(matches.is_empty());
}

#[tokio::test]
async fn by_symbol_lowercase_still_matches_curated_ticker() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("action", "getsourcecode"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__token_search__usdc.json"),
            "application/json",
        ))
        .mount(&server)
        .await;
    let adapter = adapter_for(&server.uri());

    let matches = adapter
        .by_symbol("usdc", Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].symbol, "USDC");
}

#[tokio::test]
async fn by_symbol_degrades_to_curated_name_on_rate_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("action", "getsourcecode"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__token_search__rate_limit.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let matches = adapter
        .by_symbol("USDC", Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].name, "USD Coin",
        "rate-limited response must fall back to the curated name",
    );
}

#[tokio::test]
async fn by_name_matches_curated_entry_without_http() {
    let server = MockServer::start().await;
    // No mounts — any HTTP call fails the test.
    let adapter = adapter_for(&server.uri());

    let matches = adapter
        .by_name("usd coin", Chain::Ethereum)
        .await
        .expect("ok");

    assert!(
        matches.iter().any(|m| m.symbol == "USDC"),
        "USDC must match a case-insensitive 'usd coin' query",
    );
}

#[tokio::test]
async fn by_name_with_empty_query_returns_empty() {
    let server = MockServer::start().await;
    let adapter = adapter_for(&server.uri());

    let matches = adapter.by_name("   ", Chain::Ethereum).await.expect("ok");
    assert!(matches.is_empty());
}
