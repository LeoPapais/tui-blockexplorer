//! Functional tests for `AlchemyHealth`.
//!
//! See `plan/10-settings.md` section 12.5.

use blockexplorer_tui::{
    adapters::rpc::{ALCHEMY_PROVIDER, AlchemyHealth, RpcClient},
    application::ports::HealthLevel,
    domain::Chain,
};
use serde_json::json;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

use crate::support::fixture_loader::load_text;

async fn health_for(
    body_fixture: &str,
    chain: Chain,
) -> blockexplorer_tui::application::ports::HealthStatus {
    let server = MockServer::start().await;
    let body = load_text(body_fixture);
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method": "eth_chainId"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;

    let base_url = Url::parse(&server.uri()).unwrap();
    let http = reqwest::Client::new();
    let probe = AlchemyHealth::new(RpcClient::new(base_url, http));
    probe
        .probe(chain)
        .await
        .expect("probe must return a status even on failure paths")
}

#[tokio::test]
async fn healthy_when_chain_id_matches() {
    let status = health_for("health__alchemy__eth_chainId_ok.json", Chain::Ethereum).await;

    assert_eq!(status.status, HealthLevel::Healthy);
    assert_eq!(status.provider, ALCHEMY_PROVIDER);
}

#[tokio::test]
async fn degraded_when_chain_id_mismatches() {
    let status = health_for(
        "health__alchemy__eth_chainId_mismatch.json",
        Chain::Ethereum,
    )
    .await;

    assert_eq!(status.status, HealthLevel::Degraded);
    assert!(
        status
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("expected chain id")
    );
}

#[tokio::test]
async fn down_when_provider_errors_out() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let base_url = Url::parse(&server.uri()).unwrap();
    let probe = AlchemyHealth::new(RpcClient::new(base_url, reqwest::Client::new()));
    let status = probe.probe(Chain::Ethereum).await.expect("status");

    assert_eq!(status.status, HealthLevel::Down);
}
