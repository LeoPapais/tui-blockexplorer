//! Wiremock test for the Alchemy storage adapter.
//!
//! See `plan/7-contract-detail.md` section 12.4.3.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyStorage, RpcClient},
    application::ports::StoragePort,
    domain::{Address, Chain},
};
use url::Url;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

use crate::support::fixture_loader::load_text;

fn adapter_for(url: &str) -> AlchemyStorage {
    let rpc = RpcClient::new(Url::parse(url).unwrap(), reqwest::Client::new());
    AlchemyStorage::new(rpc)
}

#[tokio::test]
async fn decodes_storage_word() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("alchemy__eth_getStorageAt__slot0.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let contract = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();

    let word = adapter
        .get_at(contract, Chain::Ethereum, [0u8; 32])
        .await
        .expect("ok");
    assert_eq!(word[31], 0x42);
    assert!(word[..31].iter().all(|b| *b == 0));
}
