//! Wiremock-driven tests for `AlchemyProxyDetector`.
//!
//! Covers the three slots probed by the detector (plan/7 §12.5.2):
//! EIP-1967 impl, EIP-1822 PROXIABLE (UUPS), Transparent admin. Each
//! slot is disambiguated in wiremock via `body_string_contains` on
//! the slot hex so the tests do not depend on wiremock request
//! ordering.
//!
//! See `plan/7-contract-detail.md` sections 12.2 and 12.5.2.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyProxyDetector, RpcClient},
    application::ports::ProxyDetectionPort,
    domain::{Address, Chain, ProxyKind, ProxySource},
};
use serde_json::json;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, body_string_contains, method},
};

use crate::support::fixture_loader::load_text;

const EIP1967_SLOT: &str = "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc";
const EIP1822_SLOT: &str = "0xc5f16f0fcc639fa48a6947836d9850f504798523bf8c9a3a87d5876cf622bcf7";
const TRANSPARENT_SLOT: &str = "0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103";

fn detector_for(url: &str) -> AlchemyProxyDetector {
    AlchemyProxyDetector::new(RpcClient::new(
        Url::parse(url).unwrap(),
        reqwest::Client::new(),
    ))
}

async fn mount_zero_fallback(server: &MockServer) {
    // Default: any eth_getStorageAt call returns zero. Tests that
    // want a specific slot to resolve mount a more specific mock
    // (priority 1) *before* this one (priority 100) so wiremock
    // picks the specific one first.
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getStorageAt"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getStorageAt__eip1967_zero.json"),
            "application/json",
        ))
        .with_priority(100)
        .mount(server)
        .await;
}

#[tokio::test]
async fn zero_slot_reports_no_proxy() {
    let server = MockServer::start().await;
    mount_zero_fallback(&server).await;

    let detector = detector_for(&server.uri());
    let addr = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    let got = detector.detect(addr, Chain::Ethereum).await.expect("ok");
    assert!(got.is_none());
}

#[tokio::test]
async fn eip1967_slot_decodes_implementation() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains(EIP1967_SLOT))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getStorageAt__eip1967_impl.json"),
            "application/json",
        ))
        .with_priority(1)
        .mount(&server)
        .await;
    mount_zero_fallback(&server).await;

    let detector = detector_for(&server.uri());
    let addr = Address::from_hex("0xa0a1000000000000000000000000000000000001").unwrap();
    let info = detector
        .detect(addr, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("proxy detected");

    assert_eq!(info.kind, ProxyKind::Eip1967);
    assert_eq!(info.source, ProxySource::Eip1967Slot);
    assert_eq!(
        info.implementation.to_hex(),
        "0xb0b1000000000000000000000000000000000002"
    );
}

#[tokio::test]
async fn uups_slot_decodes_implementation_when_eip1967_is_zero() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains(EIP1822_SLOT))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getStorageAt__uups_impl.json"),
            "application/json",
        ))
        .with_priority(1)
        .mount(&server)
        .await;
    mount_zero_fallback(&server).await;

    let detector = detector_for(&server.uri());
    let addr = Address::from_hex("0xa0a1000000000000000000000000000000000002").unwrap();
    let info = detector
        .detect(addr, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("proxy detected");

    assert_eq!(info.kind, ProxyKind::Uups);
    assert_eq!(info.source, ProxySource::Eip1822Slot);
    assert_eq!(
        info.implementation.to_hex(),
        "0xc0c1000000000000000000000000000000000003"
    );
}

#[tokio::test]
async fn transparent_admin_slot_triggers_transparent_detection() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains(TRANSPARENT_SLOT))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getStorageAt__transparent_admin.json"),
            "application/json",
        ))
        .with_priority(1)
        .mount(&server)
        .await;
    mount_zero_fallback(&server).await;

    let detector = detector_for(&server.uri());
    let addr = Address::from_hex("0xa0a1000000000000000000000000000000000003").unwrap();
    let info = detector
        .detect(addr, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("proxy detected");

    assert_eq!(info.kind, ProxyKind::Transparent);
    assert_eq!(info.source, ProxySource::TransparentSlot);
    assert_eq!(
        info.implementation.to_hex(),
        "0xadadadadadadadadadadadadadadadadadadadad"
    );
}

#[tokio::test]
async fn eip1967_wins_over_other_slots_when_multiple_are_populated() {
    // Guard against regressions in the slot probe order. If the
    // detector ever starts checking UUPS first, this test catches
    // it because we report the EIP-1967 address while every slot
    // is non-zero.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains(EIP1967_SLOT))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getStorageAt__eip1967_impl.json"),
            "application/json",
        ))
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains(EIP1822_SLOT))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getStorageAt__uups_impl.json"),
            "application/json",
        ))
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains(TRANSPARENT_SLOT))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getStorageAt__transparent_admin.json"),
            "application/json",
        ))
        .with_priority(1)
        .mount(&server)
        .await;

    let detector = detector_for(&server.uri());
    let addr = Address::from_hex("0xa0a1000000000000000000000000000000000004").unwrap();
    let info = detector
        .detect(addr, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("proxy detected");

    assert_eq!(info.kind, ProxyKind::Eip1967);
}

#[tokio::test]
async fn rpc_error_surfaces_as_domain_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__error__generic_server.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let detector = detector_for(&server.uri());
    let addr = Address::from_hex("0xa0a1000000000000000000000000000000000005").unwrap();
    let err = detector
        .detect(addr, Chain::Ethereum)
        .await
        .expect_err("primary error must propagate");

    // `-32000 server error` falls in the JSON-RPC server-defined
    // range (`-32099..=-32000`) and therefore maps to
    // `DomainError::ProviderUnavailable` under the §8.1 mapping
    // (plan/13-alchemy-adapter.md §8.1 / plan/15-backlog.md §8.14
    // item 6).
    assert!(matches!(
        err,
        blockexplorer_tui::domain::DomainError::ProviderUnavailable
    ));
}
