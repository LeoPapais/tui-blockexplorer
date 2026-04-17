//! Functional tests for `AlchemyGasOracleAdapter` against wiremock.
//!
//! See `plan/13-alchemy-adapter.md` sections 3.2 and 5.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyGasOracleAdapter, RpcClient},
    application::ports::GasOraclePort,
    domain::{Chain, DomainError},
};
use serde_json::json;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

use crate::support::fixture_loader::load_text;

fn adapter_for(url: &str) -> AlchemyGasOracleAdapter {
    let client = RpcClient::new(
        Url::parse(url).unwrap(),
        reqwest::Client::new(),
    );
    AlchemyGasOracleAdapter::new(client)
}

#[tokio::test]
async fn happy_path_returns_tiered_snapshot() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_feeHistory"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                load_text("rpc__eth_feeHistory__ethereum_20_blocks.json"),
                "application/json",
            ),
        )
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let snapshot = adapter.snapshot(Chain::Ethereum).await.expect("ok");

    // Base fee from the last entry is 1 gwei (0x3b9aca00 wei).
    assert_eq!(snapshot.base_fee.value(), 1);
    // Tips: [0, 1, 2] gwei applied on top of the base fee.
    assert_eq!(snapshot.slow.value(), 1);
    assert_eq!(snapshot.average.value(), 2);
    assert_eq!(snapshot.fast.value(), 3);
    assert_eq!(snapshot.trend.len(), 21);
}

#[tokio::test]
async fn unsupported_method_maps_to_feature_unavailable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_feeHistory"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(load_text("rpc__error__method_not_found.json"), "application/json"),
        )
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let err = adapter
        .snapshot(Chain::Arbitrum)
        .await
        .expect_err("missing method must degrade gracefully");

    assert!(matches!(err, DomainError::FeatureUnavailable));
}

#[tokio::test]
async fn rate_limit_maps_to_provider_unavailable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_feeHistory"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(load_text("rpc__error__rate_limit.json"), "application/json"),
        )
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let err = adapter
        .snapshot(Chain::Ethereum)
        .await
        .expect_err("rate limited error must propagate");

    assert!(matches!(err, DomainError::ProviderUnavailable));
}
