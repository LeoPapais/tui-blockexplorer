//! Wiremock tests for `AlchemyTransfers::get_for_contract`.
//!
//! See `plan/8-token-detail.md` section 12.4.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyTransfers, RpcClient},
    application::ports::TransfersPort,
    domain::{Address, Chain, TransferAsset, TransferCategory},
};
use serde_json::{Value, json};
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

use crate::support::fixture_loader::load_text;

fn adapter_for(url: &str) -> AlchemyTransfers {
    AlchemyTransfers::new(RpcClient::new(
        Url::parse(url).unwrap(),
        reqwest::Client::new(),
    ))
}

#[tokio::test]
async fn contract_query_returns_merged_page() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({
            "method": "alchemy_getAssetTransfers"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("alchemy__asset_transfers__by_contract.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let contract = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();

    let page = adapter
        .get_for_contract(contract, Chain::Ethereum, None)
        .await
        .expect("ok");

    assert_eq!(page.events.len(), 2);
    // Sorted by block number descending.
    assert!(page.events[0].block_number.value() > page.events[1].block_number.value());
    // Every event is ERC-20 with the USDC contract.
    for e in &page.events {
        assert_eq!(e.category, TransferCategory::Erc20);
        match &e.asset {
            TransferAsset::Erc20 {
                contract: c,
                symbol,
                ..
            } => {
                assert_eq!(c, &contract);
                assert_eq!(symbol, "USDC");
            }
            other => panic!("expected ERC-20 asset, got {other:?}"),
        }
    }

    let cursor = page.next_cursor.expect("cursor present");
    assert_eq!(cursor.0, "contract-cursor");
}

#[tokio::test]
async fn request_sends_contract_addresses_and_erc20_category_only() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"jsonrpc":"2.0","id":1,"result":{"transfers":[],"pageKey":null}}"#,
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let contract = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();

    let _ = adapter
        .get_for_contract(contract, Chain::Ethereum, None)
        .await
        .expect("ok");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests.len(),
        1,
        "single RPC call for contract-scoped query"
    );
    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    let params = body.pointer("/params/0").expect("params array [0]");
    assert_eq!(
        params.get("contractAddresses").unwrap(),
        &json!([contract.to_hex()])
    );
    assert_eq!(params.get("category").unwrap(), &json!(["erc20"]));
    assert!(
        params.get("fromAddress").is_none(),
        "contract-scoped query must not also send fromAddress"
    );
    assert!(params.get("toAddress").is_none());
}
