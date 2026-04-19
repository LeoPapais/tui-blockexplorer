//! Functional tests for `EtherscanHealth`.
//!
//! See `plan/10-settings.md` section 12.5.

use blockexplorer_tui::{
    adapters::etherscan::{ETHERSCAN_PROVIDER, EtherscanClient, EtherscanHealth},
    application::ports::HealthLevel,
    domain::Chain,
};
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, query_param},
};

use crate::support::fixture_loader::load_text;

fn client(server: &MockServer) -> EtherscanClient {
    let http = reqwest::Client::new();
    let url = Url::parse(&format!("{}/api", server.uri())).unwrap();
    EtherscanClient::new(url, "test-key".to_string(), http)
}

#[tokio::test]
async fn healthy_when_status_equals_one() {
    let server = MockServer::start().await;
    let body = load_text("health__etherscan__chainsize_ok.json");
    Mock::given(method("GET"))
        .and(query_param("module", "stats"))
        .and(query_param("action", "chainsize"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;

    let probe = EtherscanHealth::new(client(&server));
    let status = probe.probe(Chain::Ethereum).await.expect("status");

    assert_eq!(status.status, HealthLevel::Healthy);
    assert_eq!(status.provider, ETHERSCAN_PROVIDER);
}

#[tokio::test]
async fn degraded_when_status_is_zero() {
    let server = MockServer::start().await;
    let body = load_text("health__etherscan__chainsize_degraded.json");
    Mock::given(method("GET"))
        .and(query_param("module", "stats"))
        .and(query_param("action", "chainsize"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;

    let probe = EtherscanHealth::new(client(&server));
    let status = probe.probe(Chain::Ethereum).await.expect("status");

    assert_eq!(status.status, HealthLevel::Degraded);
    let message = status.message.as_deref().unwrap_or_default().to_lowercase();
    assert!(
        message.contains("notok") || message.contains("rate"),
        "degraded message must carry the adapter diagnostic: {message:?}"
    );
}

#[tokio::test]
async fn down_on_http_500() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let probe = EtherscanHealth::new(client(&server));
    let status = probe.probe(Chain::Ethereum).await.expect("status");

    // A 500 returns a non-JSON body; the adapter surfaces the Down
    // level regardless of the exact decode path (http error / decode
    // error).
    assert_eq!(status.status, HealthLevel::Down);
}
