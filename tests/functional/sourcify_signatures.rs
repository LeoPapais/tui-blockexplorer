//! Wiremock-driven tests for `SourcifySignatureDirectory`.
//!
//! See `plan/4-tx-detail.md` section 12.4.1.

use blockexplorer_tui::{
    adapters::signatures::SourcifySignatureDirectory, application::ports::SignatureDirectoryPort,
};
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

use crate::support::fixture_loader::load_text;

fn adapter_for(url: &str) -> SourcifySignatureDirectory {
    SourcifySignatureDirectory::new(reqwest::Client::new(), Url::parse(url).unwrap())
}

#[tokio::test]
async fn selector_lookup_returns_text_signature() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/signatures"))
        .and(query_param("hex_signature", "0xa9059cbb"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("sourcify__signatures__transfer.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let got = adapter
        .lookup_selector([0xa9, 0x05, 0x9c, 0xbb])
        .await
        .expect("ok")
        .expect("signature");

    assert_eq!(got, "transfer(address,uint256)");
}

#[tokio::test]
async fn selector_lookup_returns_none_on_empty_results() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/signatures"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(load_text("sourcify__signatures__empty.json"), "application/json"),
        )
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let got = adapter
        .lookup_selector([0xde, 0xad, 0xbe, 0xef])
        .await
        .expect("ok");
    assert!(got.is_none());
}

#[tokio::test]
async fn event_topic_lookup_returns_signature() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/event-signatures"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                load_text("sourcify__event_signatures__transfer.json"),
                "application/json",
            ),
        )
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
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

    assert_eq!(got, "Transfer(address,address,uint256)");
}
