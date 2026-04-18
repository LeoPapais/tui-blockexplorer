//! Wiremock-driven tests for `EtherscanLabel`.
//!
//! See `plan/3-block-detail.md` §12.4.

use blockexplorer_tui::{
    adapters::etherscan::{EtherscanClient, EtherscanLabel},
    application::ports::LabelPort,
    domain::{Address, Chain, LabelSource},
};
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

use crate::support::fixture_loader::load_text;

fn adapter_for(url: &str) -> EtherscanLabel {
    let client = EtherscanClient::new(
        Url::parse(&format!("{url}/v2/api")).unwrap(),
        "test-key".to_string(),
        reqwest::Client::new(),
    );
    EtherscanLabel::new(client)
}

#[tokio::test]
async fn labelled_contract_returns_contract_name() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("module", "contract"))
        .and(query_param("action", "getsourcecode"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__getsourcecode__label_coinbase_hot_wallet.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let addr = Address::from_hex("0xa090e606e30bd747d4e6245a1517ebe430f0057e").unwrap();

    let label = adapter
        .label_for(addr, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("labelled");

    assert_eq!(label.name, "Coinbase 10");
    assert_eq!(label.source, LabelSource::Etherscan);
}

#[tokio::test]
async fn unverified_contract_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/api"))
        .and(query_param("module", "contract"))
        .and(query_param("action", "getsourcecode"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("etherscan__getsourcecode__label_unverified.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let addr = Address::from_hex("0xdeadbeef00000000000000000000000000000000").unwrap();

    let label = adapter
        .label_for(addr, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(label, None);
}
