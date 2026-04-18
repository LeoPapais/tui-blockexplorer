//! Wiremock-driven tests for `AlchemyProxyDetector`.
//!
//! See `plan/7-contract-detail.md` section 12.2.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyProxyDetector, RpcClient},
    application::ports::ProxyDetectionPort,
    domain::{Address, Chain, ProxyKind},
};
use serde_json::json;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

use crate::support::fixture_loader::load_text;

fn detector_for(url: &str) -> AlchemyProxyDetector {
    AlchemyProxyDetector::new(RpcClient::new(
        Url::parse(url).unwrap(),
        reqwest::Client::new(),
    ))
}

#[tokio::test]
async fn zero_slot_reports_no_proxy() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getStorageAt"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getStorageAt__eip1967_zero.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let detector = detector_for(&server.uri());
    let addr = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    let got = detector.detect(addr, Chain::Ethereum).await.expect("ok");
    assert!(got.is_none());
}

#[tokio::test]
async fn populated_slot_decodes_implementation() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getStorageAt"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getStorageAt__eip1967_impl.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let detector = detector_for(&server.uri());
    let addr = Address::from_hex("0xa0a1000000000000000000000000000000000001").unwrap();
    let info = detector
        .detect(addr, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("proxy detected");

    assert_eq!(info.kind, ProxyKind::Eip1967);
    assert_eq!(
        info.implementation.to_hex(),
        "0xb0b1000000000000000000000000000000000002"
    );
}
