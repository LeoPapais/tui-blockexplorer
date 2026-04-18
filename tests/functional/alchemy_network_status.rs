//! Functional tests for the `AlchemyNetworkStatusAdapter` against a
//! wiremock server serving canned JSON-RPC responses.
//!
//! See `plan/13-alchemy-adapter.md` sections 3.1 and 5.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyNetworkStatusAdapter, RpcClient},
    application::ports::NetworkStatusPort,
    domain::{Chain, DomainError},
};
use serde_json::json;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

use crate::support::fixture_loader::load_text;

async fn server_with(body_matcher: serde_json::Value, fixture_path: &str) -> MockServer {
    let server = MockServer::start().await;
    let body = load_text(fixture_path);
    Mock::given(method("POST"))
        .and(body_partial_json(body_matcher))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
    server
}

async fn server_with_two(
    first: (serde_json::Value, &str),
    second: (serde_json::Value, &str),
) -> MockServer {
    let server = MockServer::start().await;

    let first_body = load_text(first.1);
    Mock::given(method("POST"))
        .and(body_partial_json(first.0))
        .respond_with(ResponseTemplate::new(200).set_body_raw(first_body, "application/json"))
        .mount(&server)
        .await;

    let second_body = load_text(second.1);
    Mock::given(method("POST"))
        .and(body_partial_json(second.0))
        .respond_with(ResponseTemplate::new(200).set_body_raw(second_body, "application/json"))
        .mount(&server)
        .await;

    server
}

fn adapter_for(url: &str) -> AlchemyNetworkStatusAdapter {
    let client = RpcClient::new(Url::parse(url).unwrap(), reqwest::Client::new());
    AlchemyNetworkStatusAdapter::new(client)
}

#[tokio::test]
async fn happy_path_returns_head_block_and_base_fee() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_blockNumber"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_blockNumber__ethereum.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockByNumber"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBlockByNumber__ethereum_head.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockByHash"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBlockByHash__ethereum_parent.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let status = adapter.snapshot(Chain::Ethereum).await.expect("ok");

    assert_eq!(status.chain, Chain::Ethereum);
    assert_eq!(status.latest_block.value(), 0x10);
    assert_eq!(status.base_fee.value(), 0x3b9aca00);
    // head ts 0x64 = 100, parent 0x5a = 90, diff 10s -> 10_000 ms.
    assert_eq!(status.block_time_avg_ms, 10_000);
}

#[tokio::test]
async fn rate_limit_maps_to_provider_unavailable() {
    let server = server_with(
        json!({"method":"eth_blockNumber"}),
        "rpc__error__rate_limit.json",
    )
    .await;
    let adapter = adapter_for(&server.uri());

    let err = adapter
        .snapshot(Chain::Ethereum)
        .await
        .expect_err("rate limited error must propagate");

    assert!(matches!(err, DomainError::ProviderUnavailable));
}

#[tokio::test]
async fn http_429_maps_to_provider_unavailable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_blockNumber"})))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let err = adapter
        .snapshot(Chain::Ethereum)
        .await
        .expect_err("HTTP 429 must be rate limited");

    assert!(matches!(err, DomainError::ProviderUnavailable));
}

#[allow(dead_code)]
async fn _unused_helper_for_future_tests() {
    let _ = server_with_two;
}
