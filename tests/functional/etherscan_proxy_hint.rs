//! Wiremock-driven tests for `EtherscanProxyHint`.
//!
//! See `plan/7-contract-detail.md` section 12.5.1.

use blockexplorer_tui::{
    adapters::etherscan::{EtherscanClient, EtherscanProxyHint},
    application::ports::EtherscanProxyHintPort,
    domain::{Address, Chain},
};
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

use crate::support::fixture_loader::load_text;

fn adapter_for(url: &str) -> EtherscanProxyHint {
    let client = EtherscanClient::new(
        Url::parse(&format!("{url}/v2/api")).unwrap(),
        "test-key".to_string(),
        reqwest::Client::new(),
    );
    EtherscanProxyHint::new(client)
}

#[tokio::test]
async fn proxy_contract_returns_implementation() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("module", "contract"))
        .and(query_param("action", "getsourcecode"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__getsourcecode__proxy_with_implementation.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let addr = Address::from_hex("0xe6a537a407488807f0bbeb0038b79004f19dddfb").unwrap();
    let got = adapter
        .implementation_hint(addr, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("implementation hint present");

    assert_eq!(got.to_hex(), "0xb0b1000000000000000000000000000000000099");
}

#[tokio::test]
async fn non_proxy_contract_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("action", "getsourcecode"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__getsourcecode__proxy_without_implementation.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let addr = Address::from_hex("0x0000000000000000000000000000000000000042").unwrap();
    let got = adapter
        .implementation_hint(addr, Chain::Ethereum)
        .await
        .expect("ok");

    assert!(got.is_none());
}

#[tokio::test]
async fn provider_error_surfaces() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("action", "getsourcecode"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__getsourcecode__proxy_rate_limited.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let addr = Address::from_hex("0x0000000000000000000000000000000000000042").unwrap();
    let err = adapter
        .implementation_hint(addr, Chain::Ethereum)
        .await
        .expect_err("rate-limit row must error");

    assert!(format!("{err:?}").contains("rate limit"));
}
