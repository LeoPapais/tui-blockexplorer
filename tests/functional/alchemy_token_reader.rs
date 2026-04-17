//! Wiremock-driven tests for `AlchemyTokenReader`.
//!
//! See `plan/8-token-detail.md` section 12.2.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyTokenReader, RpcClient},
    application::ports::TokenReaderPort,
    domain::{Address, Chain},
};
use serde_json::json;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

use crate::support::fixture_loader::load_text;

fn reader_for(url: &str) -> AlchemyTokenReader {
    AlchemyTokenReader::new(RpcClient::new(
        Url::parse(url).unwrap(),
        reqwest::Client::new(),
    ))
}

#[tokio::test]
async fn happy_path_returns_overview() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"alchemy_getTokenMetadata"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                load_text("rpc__alchemy_getTokenMetadata__usdc.json"),
                "application/json",
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_call"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                load_text("rpc__eth_call__totalSupply_usdc.json"),
                "application/json",
            ),
        )
        .mount(&server)
        .await;

    let reader = reader_for(&server.uri());
    let addr = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    let ov = reader
        .get(addr, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("found");

    assert_eq!(ov.metadata.symbol, "USDC");
    assert_eq!(ov.metadata.name, "USD Coin");
    assert_eq!(ov.metadata.decimals, 6);
    // 0x2000fa48e800 = 35_188_571_170_816
    assert_eq!(ov.total_supply, 35_188_571_170_816);
}

#[tokio::test]
async fn missing_metadata_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"alchemy_getTokenMetadata"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                load_text("rpc__alchemy_getTokenMetadata__missing.json"),
                "application/json",
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_call"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                load_text("rpc__eth_call__totalSupply_usdc.json"),
                "application/json",
            ),
        )
        .mount(&server)
        .await;

    let reader = reader_for(&server.uri());
    let addr = Address::from_hex("0x0000000000000000000000000000000000000001").unwrap();
    let got = reader.get(addr, Chain::Ethereum).await.expect("ok");
    assert!(got.is_none());
}
