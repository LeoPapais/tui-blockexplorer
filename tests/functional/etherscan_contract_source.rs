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
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__getabi__usdc.json"),
            "application/json",
        ))
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

#[tokio::test]
async fn single_file_source_is_surfaced_as_one_file() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("action", "getsourcecode"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__getsourcecode__verified_single_file.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let addr = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();

    let source = adapter
        .get_source(addr, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("verified");

    assert!(source.is_verified);
    assert_eq!(source.contract_name, "Storage");
    assert!(source.optimizer_enabled);
    assert_eq!(source.optimizer_runs, 200);
    assert_eq!(source.files.len(), 1);
    assert_eq!(source.files[0].path, "Storage.sol");
    assert!(source.files[0].content.contains("contract Storage"));
    assert!(source.implementation.is_none());
}

#[tokio::test]
async fn multi_file_source_parses_the_double_wrapped_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("action", "getsourcecode"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__getsourcecode__verified_multi_file.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let addr = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();

    let source = adapter
        .get_source(addr, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("verified");

    assert_eq!(source.files.len(), 2);
    let paths: Vec<_> = source.files.iter().map(|f| f.path.clone()).collect();
    assert!(paths.contains(&"contracts/Lib.sol".to_string()));
    assert!(paths.contains(&"contracts/Token.sol".to_string()));
    assert_eq!(
        source.implementation.map(|a| a.to_hex()),
        Some("0xb0b1000000000000000000000000000000000001".to_string())
    );
}

#[tokio::test]
async fn unverified_source_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("action", "getsourcecode"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__getsourcecode__not_verified.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let addr = Address::from_hex("0x0000000000000000000000000000000000000099").unwrap();

    let got = adapter.get_source(addr, Chain::Ethereum).await.expect("ok");
    assert!(got.is_none());
}
