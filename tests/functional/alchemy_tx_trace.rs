//! Wiremock tests for the Alchemy `trace_replayTransaction` adapter.
//!
//! See `plan/4-tx-detail.md` section 12.4.3.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyTxTracer, RpcClient},
    application::ports::TxTracePort,
    domain::{Chain, DiffChange, DomainError, TxHash},
};
use url::Url;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

use crate::support::fixture_loader::load_text;

fn adapter_for(url: &str) -> AlchemyTxTracer {
    let rpc = RpcClient::new(Url::parse(url).unwrap(), reqwest::Client::new());
    AlchemyTxTracer::new(rpc)
}

#[tokio::test]
async fn parses_state_diff_into_address_entries() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                load_text("alchemy__trace_replay__state_diff.json"),
                "application/json",
            ),
        )
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let hash = TxHash::from_hex(
        "0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa",
    )
    .unwrap();

    let diff = adapter.state_diff(hash, Chain::Ethereum).await.expect("ok");
    assert_eq!(diff.entries.len(), 2);

    let eoa = diff
        .entries
        .iter()
        .find(|e| e.address.to_hex() == "0xd8da6bf26964af9d7eed9e03e53415d37aa96045")
        .expect("eoa entry");
    match &eoa.balance {
        DiffChange::Changed { from, to } => {
            assert_eq!(from, "0xde0b6b3a7640000");
            assert_eq!(to, "0xde0b6b3a7630000");
        }
        other => panic!("expected balance change, got {other:?}"),
    }
    match &eoa.nonce {
        DiffChange::Changed { from, to } => {
            assert_eq!(from, "0x2a");
            assert_eq!(to, "0x2b");
        }
        other => panic!("expected nonce change, got {other:?}"),
    }

    let contract = diff
        .entries
        .iter()
        .find(|e| e.address.to_hex() == "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
        .expect("contract entry");
    assert_eq!(contract.storage.len(), 1);
    match &contract.storage[0].change {
        DiffChange::Changed { from, to } => {
            assert!(from.ends_with("10"));
            assert!(to.ends_with("20"));
        }
        other => panic!("expected storage change, got {other:?}"),
    }
}

#[tokio::test]
async fn method_not_found_maps_to_feature_unavailable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("alchemy__trace_replay__method_not_found.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let hash = TxHash::from_hex(
        "0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa",
    )
    .unwrap();

    let err = adapter
        .state_diff(hash, Chain::Ethereum)
        .await
        .expect_err("must surface unavailability");

    assert!(matches!(err, DomainError::FeatureUnavailable));
}
