//! Wiremock-driven tests for [`HttpSignatureDirectory`] against the
//! openchain-shape API (base `https://api.openchain.xyz`).
//!
//! Mirrors the fallback-chain spec in `plan/15-backlog.md` section
//! 3.2 (openchain is the primary directory after the Sourcify mirror
//! went dark). The Samczsun adapter reuses the same wire code through
//! `SamczsunSignatureDirectory` and is exercised by the composite
//! tests.

use blockexplorer_tui::{
    adapters::signatures::HttpSignatureDirectory,
    application::{
        SignatureSource,
        ports::{SignatureDirectoryPort, SignatureHit},
    },
    domain::DomainError,
};
use pretty_assertions::assert_eq;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

use crate::support::fixture_loader::load_text;

fn adapter_for(server: &MockServer, http: &reqwest::Client) -> HttpSignatureDirectory {
    HttpSignatureDirectory::new(
        http.clone(),
        Url::parse(&server.uri()).unwrap(),
        SignatureSource::Openchain,
    )
}

#[tokio::test]
async fn selector_hit_returns_openchain_signature_with_provenance() {
    let http = reqwest::Client::builder().build().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/signature-database/v1/lookup"))
        .and(query_param("function", "0xa9059cbb"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("openchain__lookup__transfer_hit.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server, &http);
    let got = adapter
        .lookup_selector([0xa9, 0x05, 0x9c, 0xbb])
        .await
        .expect("ok")
        .expect("signature");

    assert_eq!(
        got,
        SignatureHit {
            signature: "transfer(address,uint256)".to_string(),
            source: SignatureSource::Openchain,
        }
    );
}

#[tokio::test]
async fn selector_miss_returns_none_on_empty_function_map() {
    let http = reqwest::Client::builder().build().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/signature-database/v1/lookup"))
        .and(query_param("function", "0xdeadbeef"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("openchain__lookup__transfer_miss.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server, &http);
    let got = adapter
        .lookup_selector([0xde, 0xad, 0xbe, 0xef])
        .await
        .expect("ok");

    assert!(got.is_none());
}

#[tokio::test]
async fn event_topic_hit_returns_signature_from_event_map() {
    let http = reqwest::Client::builder().build().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/signature-database/v1/lookup"))
        .and(query_param(
            "event",
            "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("openchain__lookup__transfer_event_hit.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server, &http);
    let mut topic = [0u8; 32];
    hex::decode_to_slice(
        "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
        &mut topic,
    )
    .unwrap();

    let got = adapter
        .lookup_event_topic(topic)
        .await
        .expect("ok")
        .expect("signature");

    assert_eq!(got.signature, "Transfer(address,address,uint256)");
    assert_eq!(got.source, SignatureSource::Openchain);
}

#[tokio::test]
async fn filtered_entries_are_skipped_in_favour_of_first_non_filtered() {
    // Craft a response whose first entry is filtered; the adapter
    // must keep scanning until it hits the canonical signature.
    let http = reqwest::Client::builder().build().unwrap();
    let server = MockServer::start().await;
    let body = r#"{
        "ok": true,
        "result": {
            "event": {},
            "function": {
                "0xa9059cbb": [
                    {"name": "spam(bytes)", "filtered": true},
                    {"name": "transfer(address,uint256)", "filtered": false}
                ]
            }
        }
    }"#;
    Mock::given(method("GET"))
        .and(path("/signature-database/v1/lookup"))
        .and(query_param("function", "0xa9059cbb"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server, &http);
    let got = adapter
        .lookup_selector([0xa9, 0x05, 0x9c, 0xbb])
        .await
        .expect("ok")
        .expect("signature");

    assert_eq!(got.signature, "transfer(address,uint256)");
}

#[tokio::test]
async fn http_500_maps_to_provider_unavailable_or_internal() {
    let http = reqwest::Client::builder().build().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/signature-database/v1/lookup"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server, &http);
    let got = adapter
        .lookup_selector([0xa9, 0x05, 0x9c, 0xbb])
        .await
        .expect("non-success body must not raise a DomainError");

    // Per adapter contract, non-2xx responses become `Ok(None)`
    // rather than errors: the openchain mirror has historically
    // returned 500s when the backend is warming up, and we do not
    // want that to block the tx-detail render. Error mapping is
    // exercised separately when the transport itself fails.
    assert!(got.is_none());

    // Now exercise transport-level failure: aim the adapter at a
    // port that is not bound, so the client cannot connect.
    let dead_adapter = HttpSignatureDirectory::new(
        http,
        Url::parse("http://127.0.0.1:1").unwrap(),
        SignatureSource::Openchain,
    );
    let err = dead_adapter
        .lookup_selector([0xa9, 0x05, 0x9c, 0xbb])
        .await
        .expect_err("connection refused must surface as DomainError");
    assert!(matches!(
        err,
        DomainError::ProviderUnavailable | DomainError::Internal(_)
    ));
}
