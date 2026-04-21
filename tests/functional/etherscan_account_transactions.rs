//! Wiremock-driven tests for `EtherscanAccountTransactions`.
//!
//! See `plan/18-shell-navigation-and-feeds.md` Slice D.

use assert_matches::assert_matches;
use blockexplorer_tui::{
    adapters::etherscan::{EtherscanAccountTransactions, EtherscanClient},
    application::ports::AccountTransactionsPort,
    domain::{Address, Chain, DomainError},
};
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

use crate::support::fixture_loader::load_text;

fn adapter_for(url: &str) -> EtherscanAccountTransactions {
    let client = EtherscanClient::new(
        Url::parse(&format!("{url}/v2/api")).unwrap(),
        "test-key".to_string(),
        reqwest::Client::new(),
    );
    EtherscanAccountTransactions::new(client)
}

#[tokio::test]
async fn txlist_returns_two_rows_newest_first() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("module", "account"))
        .and(query_param("action", "txlist"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__txlist__two_txs.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let addr = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

    let page = adapter
        .list_for_address(addr, Chain::Ethereum, None)
        .await
        .expect("ok");
    assert_eq!(page.txs.len(), 2);
    assert_eq!(page.txs[0].block_number.value(), 18_500_001);
    assert_eq!(page.txs[1].block_number.value(), 18_500_000);
    assert!(page.next_cursor.is_none());
}

#[tokio::test]
async fn notok_string_result_maps_to_internal_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("action", "txlist"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__txlist__notok.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let addr = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

    let err = adapter
        .list_for_address(addr, Chain::Ethereum, None)
        .await
        .expect_err("NOTOK must fail");
    assert_matches!(err, DomainError::Internal(_));
}

#[tokio::test]
async fn http_5xx_maps_to_provider_unavailable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_raw(load_text("etherscan__error__5xx.json"), "application/json"),
        )
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let addr = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

    let err = adapter
        .list_for_address(addr, Chain::Ethereum, None)
        .await
        .expect_err("5xx");
    assert_matches!(err, DomainError::ProviderUnavailable);
}
