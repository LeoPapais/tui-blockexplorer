//! Wiremock tests for the Alchemy `trace_replayTransaction` and
//! `trace_transaction` / `debug_traceTransaction` adapters.
//!
//! See `plan/4-tx-detail.md` sections 12.4.3 (state diff) and
//! 12.6.5 (call tree).

use blockexplorer_tui::{
    adapters::rpc::{AlchemyTxTracer, RpcClient},
    application::ports::TxTracePort,
    domain::{CallKind, Chain, DiffChange, DomainError, TxHash},
};
use serde_json::Value;
use url::Url;
use wiremock::{Mock, MockServer, Request, ResponseTemplate, matchers::method};

use crate::support::fixture_loader::load_text;

fn adapter_for(url: &str) -> AlchemyTxTracer {
    let rpc = RpcClient::new(Url::parse(url).unwrap(), reqwest::Client::new());
    AlchemyTxTracer::new(rpc)
}

#[tokio::test]
async fn parses_state_diff_into_address_entries() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("alchemy__trace_replay__state_diff.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let hash =
        TxHash::from_hex("0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa")
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
    let hash =
        TxHash::from_hex("0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa")
            .unwrap();

    let err = adapter
        .state_diff(hash, Chain::Ethereum)
        .await
        .expect_err("must surface unavailability");

    assert!(matches!(err, DomainError::FeatureUnavailable));
}

// ---------------------------------------------------------------------------
// Plan 12.6.5 — `TxTracePort::call_tree`
// ---------------------------------------------------------------------------

fn responder_by_method(req: &Request) -> Option<ResponseTemplate> {
    let body: Value = serde_json::from_slice(&req.body).ok()?;
    let m = body.get("method").and_then(Value::as_str)?;
    let fixture = match m {
        "trace_transaction" => "alchemy__trace_transaction__usdc_transfer.json",
        "debug_traceTransaction" => "alchemy__debug_trace_calltracer__usdc_transfer.json",
        _ => return None,
    };
    Some(ResponseTemplate::new(200).set_body_raw(load_text(fixture), "application/json"))
}

#[tokio::test]
async fn call_tree_uses_parity_namespace_when_available() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(move |req: &Request| {
            responder_by_method(req).unwrap_or_else(|| ResponseTemplate::new(500))
        })
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let hash =
        TxHash::from_hex("0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa")
            .unwrap();

    let tree = adapter.call_tree(hash, Chain::Ethereum).await.expect("ok");
    assert_eq!(tree.kind, CallKind::Call);
    assert_eq!(tree.children.len(), 1);
    assert_eq!(tree.children[0].kind, CallKind::Staticcall);
    assert_eq!(tree.frame_count(), 2);
    assert_eq!(tree.gas_used, 0x7fff);
    assert_eq!(tree.children[0].gas_used, 0x100);
}

#[tokio::test]
async fn call_tree_falls_back_to_debug_tracer() {
    let server = MockServer::start().await;
    // First POST (trace_transaction) returns method-not-found;
    // second POST (debug_traceTransaction) returns the call tree.
    Mock::given(method("POST"))
        .respond_with(move |req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let m = body.get("method").and_then(Value::as_str).unwrap_or("");
            match m {
                "trace_transaction" => ResponseTemplate::new(200).set_body_raw(
                    load_text("alchemy__trace_transaction__method_not_found.json"),
                    "application/json",
                ),
                "debug_traceTransaction" => ResponseTemplate::new(200).set_body_raw(
                    load_text("alchemy__debug_trace_calltracer__usdc_transfer.json"),
                    "application/json",
                ),
                _ => ResponseTemplate::new(500),
            }
        })
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let hash =
        TxHash::from_hex("0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa")
            .unwrap();

    let tree = adapter.call_tree(hash, Chain::Ethereum).await.expect("ok");
    assert_eq!(tree.kind, CallKind::Call);
    assert_eq!(tree.children.len(), 1);
    assert_eq!(tree.children[0].kind, CallKind::Staticcall);
}

#[tokio::test]
async fn call_tree_fails_when_both_tracers_unsupported() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(move |req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let m = body.get("method").and_then(Value::as_str).unwrap_or("");
            match m {
                "trace_transaction" => ResponseTemplate::new(200).set_body_raw(
                    load_text("alchemy__trace_transaction__method_not_found.json"),
                    "application/json",
                ),
                "debug_traceTransaction" => ResponseTemplate::new(200).set_body_raw(
                    load_text("alchemy__debug_trace_calltracer__method_not_found.json"),
                    "application/json",
                ),
                _ => ResponseTemplate::new(500),
            }
        })
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let hash =
        TxHash::from_hex("0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa")
            .unwrap();

    let err = adapter
        .call_tree(hash, Chain::Ethereum)
        .await
        .expect_err("must surface unavailability when neither tracer exists");
    assert!(matches!(err, DomainError::FeatureUnavailable));
}
