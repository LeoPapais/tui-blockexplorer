//! Wiremock-driven tests for `AlchemyTxReader`.
//!
//! See `plan/4-tx-detail.md` section 12.2.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyTxReader, RpcClient},
    application::ports::TxReaderPort,
    domain::{Chain, TxHash, TxStatus, TxType},
};
use serde_json::json;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

use crate::support::fixture_loader::load_text;

fn reader_for(url: &str) -> AlchemyTxReader {
    AlchemyTxReader::new(RpcClient::new(
        Url::parse(url).unwrap(),
        reqwest::Client::new(),
    ))
}

#[tokio::test]
async fn happy_path_returns_successful_tx() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getTransactionByHash"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                load_text("rpc__eth_getTransactionByHash__usdc_transfer.json"),
                "application/json",
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getTransactionReceipt"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                load_text(
                    "rpc__eth_getTransactionReceipt__usdc_transfer_success.json",
                ),
                "application/json",
            ),
        )
        .mount(&server)
        .await;

    let reader = reader_for(&server.uri());
    let hash = TxHash::from_hex(
        "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
    )
    .unwrap();
    let tx = reader
        .get(hash, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("found");

    assert_eq!(tx.status, TxStatus::Success);
    assert_eq!(tx.block_number.value(), 0x1406f40);
    assert_eq!(tx.tx_index, 3);
    assert_eq!(tx.gas_used, 0xcc75);
    assert_eq!(tx.gas_limit, 0x13880);
    assert_eq!(tx.nonce, 42);
    assert_eq!(tx.tx_type, TxType::DynamicFee);
    assert_eq!(tx.input, vec![0xa9, 0x05, 0x9c, 0xbb]);
    assert!(tx.to.is_some());
    assert!(tx.raw_json.contains("blockNumber"));
}

#[tokio::test]
async fn reverted_tx_exposes_reason() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getTransactionByHash"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                load_text("rpc__eth_getTransactionByHash__usdc_transfer.json"),
                "application/json",
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getTransactionReceipt"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                load_text("rpc__eth_getTransactionReceipt__reverted.json"),
                "application/json",
            ),
        )
        .mount(&server)
        .await;

    let reader = reader_for(&server.uri());
    let hash = TxHash::from_hex(
        "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
    )
    .unwrap();
    let tx = reader
        .get(hash, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("found");

    match tx.status {
        TxStatus::Failed { reason } => {
            assert_eq!(reason.as_deref(), Some("InsufficientBalance()"));
        }
        TxStatus::Success => panic!("expected Failed"),
    }
}

#[tokio::test]
async fn missing_tx_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getTransactionByHash"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                load_text("rpc__eth_getTransactionByHash__null.json"),
                "application/json",
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getTransactionReceipt"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(load_text("rpc__eth_getTransactionByHash__null.json"), "application/json"),
        )
        .mount(&server)
        .await;

    let reader = reader_for(&server.uri());
    let hash = TxHash::from_hex(
        "0x0000000000000000000000000000000000000000000000000000000000000001",
    )
    .unwrap();
    let got = reader.get(hash, Chain::Ethereum).await.expect("ok");
    assert!(got.is_none());
}
