//! Wiremock-driven tests for `AlchemyBlockReader`.
//!
//! See `plan/3-block-detail.md` section 11.2.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyBlockReader, RpcClient},
    application::ports::BlockReaderPort,
    domain::{BlockHash, BlockId, BlockNumber, Chain},
};
use serde_json::json;
use pretty_assertions::assert_eq;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

use crate::support::fixture_loader::load_text;

fn reader_for(url: &str) -> AlchemyBlockReader {
    AlchemyBlockReader::new(RpcClient::new(
        Url::parse(url).unwrap(),
        reqwest::Client::new(),
    ))
}

#[tokio::test]
async fn happy_path_by_number_returns_full_block() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockByNumber"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBlockByNumber__21345678_full.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let reader = reader_for(&server.uri());
    let block = reader
        .get(
            BlockId::Number(BlockNumber::new(21_322_062)),
            Chain::Ethereum,
        )
        .await
        .expect("ok")
        .expect("found");

    assert_eq!(block.number.value(), 0x1455b4e);
    assert_eq!(
        block.hash.to_hex(),
        "0xaaaa000000000000000000000000000000000000000000000000000000000000"
    );
    assert_eq!(
        block.parent_hash.to_hex(),
        "0xbbbb000000000000000000000000000000000000000000000000000000000000"
    );
    assert_eq!(block.gas_used, 0xb71b00);
    assert_eq!(block.gas_limit, 0x1c9c380);
    assert_eq!(block.base_fee.map(|w| w.value()), Some(0x2a77e3200));
    assert_eq!(block.size, 0x19000);
    assert_eq!(block.extra_data, vec![0x42, 0x42]);
    assert_eq!(block.tx_hashes.len(), 2);
}

#[tokio::test]
async fn happy_path_by_hash_returns_full_block() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockByHash"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBlockByNumber__21345678_full.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let reader = reader_for(&server.uri());
    let block = reader
        .get(
            BlockId::Hash(
                BlockHash::from_hex(
                    "0xaaaa000000000000000000000000000000000000000000000000000000000000",
                )
                .unwrap(),
            ),
            Chain::Ethereum,
        )
        .await
        .expect("ok")
        .expect("found");

    assert_eq!(block.number.value(), 0x1455b4e);
}

#[tokio::test]
async fn parses_withdrawals_from_shanghai_block() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockByNumber"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBlockByNumber__with_withdrawals.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let reader = reader_for(&server.uri());
    let block = reader
        .get(
            BlockId::Number(BlockNumber::new(0x1455b4f)),
            Chain::Ethereum,
        )
        .await
        .expect("ok")
        .expect("found");

    assert_eq!(block.withdrawals.len(), 2);
    assert_eq!(block.withdrawals[0].index, 0x1e8480);
    assert_eq!(block.withdrawals[0].validator_index, 0xbeef);
    assert_eq!(block.withdrawals[0].amount_gwei, 0x3b9aca00);
    assert_eq!(block.withdrawals[1].validator_index, 0xf00d);
}

#[tokio::test]
async fn null_result_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockByNumber"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBlockByNumber__not_found.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let reader = reader_for(&server.uri());
    let got = reader
        .get(
            BlockId::Number(BlockNumber::new(99_999_999)),
            Chain::Ethereum,
        )
        .await
        .expect("ok");
    assert!(got.is_none());
}
