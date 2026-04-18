//! Wiremock-driven tests for `EtherscanContractSource`.
//!
//! See `plan/4-tx-detail.md` section 12.4.1.

use blockexplorer_tui::{
    adapters::etherscan::{EtherscanClient, EtherscanContractSource},
    application::ports::ContractSourcePort,
    domain::{Address, Chain},
};
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

use crate::support::fixture_loader::load_text;

fn adapter_for(url: &str) -> EtherscanContractSource {
    let client = EtherscanClient::new(
        Url::parse(&format!("{url}/v2/api")).unwrap(),
        "test-key".to_string(),
        reqwest::Client::new(),
    );
    EtherscanContractSource::new(client)
}

#[tokio::test]
async fn verified_contract_returns_abi() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("module", "contract"))
        .and(query_param("action", "getabi"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(load_text("etherscan__getabi__usdc.json"), "application/json"),
        )
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let addr = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();

    let abi = adapter
        .get_abi(addr, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("verified");

    assert!(abi.is_verified);
    assert!(abi.abi.contains("transfer"));
}

#[tokio::test]
async fn unverified_contract_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("action", "getabi"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__getabi__unverified.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let addr = Address::from_hex("0x0000000000000000000000000000000000000099").unwrap();

    let got = adapter.get_abi(addr, Chain::Ethereum).await.expect("ok");
    assert!(got.is_none());
}
