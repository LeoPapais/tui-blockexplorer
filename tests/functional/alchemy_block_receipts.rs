//! Wiremock-driven tests for `AlchemyBlockReceipts`.
//!
//! See `plan/3-block-detail.md` §12.3. The adapter issues
//! `eth_getBlockByNumber(fullTransactions=true)` and
//! `eth_getBlockReceipts` in parallel. Two fixtures cover the happy
//! path and the JSON-RPC rate-limit error surface.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyBlockReceipts, RpcClient},
    application::ports::BlockReceiptsPort,
    domain::{BlockId, BlockNumber, Chain, DomainError, TxCategory, TxStatus},
};
use serde_json::json;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

use crate::support::fixture_loader::load_text;

fn receipts_for(url: &str) -> AlchemyBlockReceipts {
    AlchemyBlockReceipts::new(RpcClient::new(
        Url::parse(url).unwrap(),
        reqwest::Client::new(),
    ))
}

#[tokio::test]
async fn zips_txs_with_receipts_and_categorises() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockByNumber"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBlockByNumber__full_txs.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockReceipts"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBlockReceipts__21345678.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let rows = receipts_for(&server.uri())
        .get_transactions(
            BlockId::Number(BlockNumber::new(0x1455b4e)),
            Chain::Ethereum,
        )
        .await
        .expect("ok");

    assert_eq!(rows.len(), 3);

    // Row 0 — 1 ETH transfer, success.
    assert_eq!(rows[0].tx_index, 0);
    assert_eq!(rows[0].value.value(), 1_000_000_000_000_000_000);
    assert!(!rows[0].has_calldata);
    assert_eq!(rows[0].category, TxCategory::Transfer);
    assert_eq!(rows[0].gas_used, Some(0x5208));
    assert!(matches!(rows[0].status, TxStatus::Success));
    assert!(rows[0].contract_address.is_none());

    // Row 1 — ERC20 transfer(...) call.
    assert_eq!(rows[1].tx_index, 1);
    assert!(rows[1].has_calldata);
    assert_eq!(rows[1].category, TxCategory::Interaction);
    assert_eq!(rows[1].gas_used, Some(0x7a6d));

    // Row 2 — contract deployment.
    assert_eq!(rows[2].tx_index, 2);
    assert!(rows[2].to.is_none());
    assert_eq!(rows[2].category, TxCategory::Deploy);
    assert!(rows[2].contract_address.is_some());
}

#[tokio::test]
async fn rate_limited_receipts_maps_to_provider_unavailable() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockByNumber"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBlockByNumber__full_txs.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockReceipts"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBlockReceipts__rate_limit.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let err = receipts_for(&server.uri())
        .get_transactions(
            BlockId::Number(BlockNumber::new(0x1455b4e)),
            Chain::Ethereum,
        )
        .await
        .expect_err("rate-limited call must surface an error");

    assert!(matches!(err, DomainError::ProviderUnavailable));
}

#[tokio::test]
async fn missing_block_maps_to_not_found() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockByNumber"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBlockByNumber__not_found.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockReceipts"})))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"jsonrpc":"2.0","id":1,"result":null}"#,
        ))
        .mount(&server)
        .await;

    let err = receipts_for(&server.uri())
        .get_transactions(
            BlockId::Number(BlockNumber::new(99_999_999)),
            Chain::Ethereum,
        )
        .await
        .expect_err("missing block must surface as NotFound");

    assert!(matches!(err, DomainError::NotFound));
}
