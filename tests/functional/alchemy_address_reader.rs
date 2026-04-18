//! Wiremock-driven tests for `AlchemyAddressReader`.
//!
//! See `plan/6-address-detail.md` section 12.2.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyAddressReader, RpcClient},
    application::ports::AddressReaderPort,
    domain::{Address, AddressKind, Chain},
};
use serde_json::json;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

use crate::support::fixture_loader::load_text;

fn reader_for(url: &str) -> AlchemyAddressReader {
    AlchemyAddressReader::new(RpcClient::new(
        Url::parse(url).unwrap(),
        reqwest::Client::new(),
    ))
}

async fn mount_common_eoa_responses(server: &MockServer) {
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBalance"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBalance__0xd8da.json"),
            "application/json",
        ))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(body_partial_json(
            json!({"method":"eth_getTransactionCount"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getTransactionCount__0xd8da.json"),
            "application/json",
        ))
        .mount(server)
        .await;
}

#[tokio::test]
async fn eoa_returns_balance_nonce_and_kind() {
    let server = MockServer::start().await;
    mount_common_eoa_responses(&server).await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getCode"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(load_text("rpc__eth_getCode__eoa.json"), "application/json"),
        )
        .mount(&server)
        .await;

    let reader = reader_for(&server.uri());
    let addr = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

    let ov = reader
        .get(addr, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("found");

    assert_eq!(ov.address, addr);
    // 0x1c5c05e5abf6b480000 = 523_140_000_000_000_000_000 wei (~523.14 ETH)
    assert_eq!(ov.balance.value(), 523_140_000_000_000_000_000u128);
    assert_eq!(ov.nonce, 0x4db);
    assert_eq!(ov.kind, AddressKind::Eoa);
    assert!(ov.ens_name.is_none());
}

#[tokio::test]
async fn contract_returns_kind_contract() {
    let server = MockServer::start().await;
    mount_common_eoa_responses(&server).await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getCode"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getCode__contract.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let reader = reader_for(&server.uri());
    let addr = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();

    let ov = reader
        .get(addr, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("found");

    assert_eq!(ov.kind, AddressKind::Contract);
}
