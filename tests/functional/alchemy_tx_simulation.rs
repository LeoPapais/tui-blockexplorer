//! Wiremock tests for the Alchemy asset-change simulation adapter.
//!
//! See `plan/4-tx-detail.md` section 12.4.3.

use blockexplorer_tui::{
    adapters::rpc::{AlchemySimulation, RpcClient},
    application::ports::TxSimulationPort,
    domain::{
        Address, AssetChangeKind, AssetKind, BlockHash, BlockNumber, Chain, DomainError,
        Transaction, TxHash, TxStatus, TxType, Wei,
    },
};
use url::Url;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

use crate::support::fixture_loader::load_text;

fn sample_tx() -> Transaction {
    Transaction {
        chain: Chain::Ethereum,
        hash: TxHash::from_hex(
            "0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa",
        )
        .unwrap(),
        status: TxStatus::Success,
        block_number: Some(BlockNumber::new(21_000_000)),
        block_hash: Some(
            BlockHash::from_hex(
                "0xaaaa000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
        ),
        tx_index: Some(0),
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()),
        value: Wei::new(1_000_000_000_000_000_000),
        gas_price: Wei::new(14_000_000_000),
        gas_used: Some(52_341),
        gas_limit: 80_000,
        nonce: 42,
        tx_type: TxType::DynamicFee,
        input: vec![0xa9, 0x05, 0x9c, 0xbb],
        logs: Vec::new(),
        raw_json: "{}".to_string(),
    }
}

fn adapter_for(url: &str) -> AlchemySimulation {
    let rpc = RpcClient::new(Url::parse(url).unwrap(), reqwest::Client::new());
    AlchemySimulation::new(rpc)
}

#[tokio::test]
async fn parses_native_and_erc20_transfers() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("alchemy__simulate_asset_changes__usdc_transfer.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let changes = adapter
        .simulate_asset_changes(&sample_tx(), Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].kind, AssetChangeKind::Transfer);
    assert!(matches!(changes[0].asset, AssetKind::Native));
    match &changes[1].asset {
        AssetKind::Erc20 {
            symbol, decimals, ..
        } => {
            assert_eq!(symbol, "USDC");
            assert_eq!(*decimals, 6);
        }
        other => panic!("expected ERC20, got {other:?}"),
    }
}

#[tokio::test]
async fn method_not_found_maps_to_feature_unavailable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("alchemy__simulate_asset_changes__method_not_found.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let err = adapter
        .simulate_asset_changes(&sample_tx(), Chain::Ethereum)
        .await
        .expect_err("must surface unavailability");

    assert!(matches!(err, DomainError::FeatureUnavailable));
}
