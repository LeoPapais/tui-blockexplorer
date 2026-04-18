//! Wiremock tests for the Alchemy `alchemy_getAssetTransfers` adapter.
//!
//! The adapter fans out two calls (fromAddress + toAddress). wiremock
//! is set up so both return a canned fixture; we assert the merged
//! + sorted output.
//!
//! See `plan/6-address-detail.md` section 12.4.1.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyTransfers, RpcClient},
    application::ports::TransfersPort,
    domain::{Address, Chain, TransferAsset, TransferCategory},
};
use serde_json::Value;
use url::Url;
use wiremock::{
    Mock, MockServer, Request, Respond, ResponseTemplate,
    matchers::{method, path},
};

use crate::support::fixture_loader::load_text;

/// Responder that inspects the JSON-RPC params: returns the `from`
/// fixture when `fromAddress` is present, otherwise the `to` fixture.
/// Matches how the real Alchemy endpoint behaves when the adapter
/// issues two queries with different address params.
struct DirectionalResponder;

impl Respond for DirectionalResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        let params = body
            .get("params")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Null);

        let fixture = if params.get("fromAddress").is_some() {
            "alchemy__asset_transfers__from.json"
        } else {
            "alchemy__asset_transfers__to.json"
        };
        ResponseTemplate::new(200)
            .set_body_raw(load_text(fixture), "application/json")
    }
}

fn adapter_for(url: &str) -> AlchemyTransfers {
    let rpc = RpcClient::new(Url::parse(url).unwrap(), reqwest::Client::new());
    AlchemyTransfers::new(rpc)
}

#[tokio::test]
async fn merges_from_and_to_transfers_sorted_desc() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(DirectionalResponder)
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let address = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

    let page = adapter
        .get_for_address(address, Chain::Ethereum, None)
        .await
        .expect("ok");

    assert_eq!(page.events.len(), 3);
    // Events are sorted by block number descending.
    assert_eq!(page.events[0].block_number.value(), 0x1406f40);
    assert_eq!(page.events[1].block_number.value(), 0x1406f30);
    assert_eq!(page.events[2].block_number.value(), 0x1406f20);

    // External ETH transfer (first event).
    assert_eq!(page.events[0].category, TransferCategory::External);
    match &page.events[0].asset {
        TransferAsset::Native { symbol } => assert_eq!(symbol, "ETH"),
        other => panic!("expected Native, got {other:?}"),
    }

    // ERC-20 transfer (second event).
    assert_eq!(page.events[1].category, TransferCategory::Erc20);
    match &page.events[1].asset {
        TransferAsset::Erc20 {
            symbol,
            decimals,
            contract,
        } => {
            assert_eq!(symbol, "USDC");
            assert_eq!(*decimals, 6);
            assert_eq!(
                contract.to_hex(),
                "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            );
        }
        other => panic!("expected Erc20, got {other:?}"),
    }

    // Cursor picks up the pageKey from the `from` fixture (the `to`
    // fixture has a null pageKey).
    let cursor = page.next_cursor.expect("cursor present");
    assert!(cursor.0.starts_with("from-cursor|"));
}

#[tokio::test]
async fn empty_responses_yield_empty_page() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(
                r#"{"jsonrpc":"2.0","id":1,"result":{"transfers":[],"pageKey":null}}"#,
            ),
        )
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let address = Address::from_hex("0x0000000000000000000000000000000000000099").unwrap();

    let page = adapter
        .get_for_address(address, Chain::Ethereum, None)
        .await
        .expect("ok");
    assert!(page.events.is_empty());
    assert!(page.next_cursor.is_none());
}
