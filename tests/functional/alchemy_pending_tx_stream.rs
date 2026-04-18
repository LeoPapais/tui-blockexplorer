//! Functional tests for [`AlchemyPendingTxStream`].
//!
//! Drives the adapter against the in-process WS harness in
//! `tests/support/ws_harness.rs` so no real network I/O happens.
//! See `plan/5-mempool.md` §11.3.4.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::rpc::AlchemyPendingTxStream,
    application::ports::PendingTxStreamPort,
    domain::{Address, Chain, PendingTxEvent, PendingTxFilter, TxHash, Wei},
};
use pretty_assertions::assert_eq;
use serde_json::json;

use crate::support::ws_harness::{CannedWsServer, WsStep};

/// Plan/5 §11.3.4 happy path: the adapter connects, subscribes with
/// `alchemy_pendingTransactions`, translates each notification into a
/// `PendingTxEvent::Added` carrying the parsed [`PendingTx`] value
/// object.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscribe_translates_alchemy_pending_notifications_into_added_events() {
    let server = CannedWsServer::start(vec![
        WsStep::ExpectSubscribeReply {
            expected_method: "eth_subscribe",
            sub_id: "0xabc123",
        },
        WsStep::EmitNotification {
            sub_id: "0xabc123",
            result: json!({
                "hash": "0xaaaa000000000000000000000000000000000000000000000000000000000001",
                "from": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
                "to": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                "value": "0x3e8",
            }),
        },
        WsStep::EmitNotification {
            sub_id: "0xabc123",
            result: json!({
                "hash": "0xaaaa000000000000000000000000000000000000000000000000000000000002",
                "from": "0x1111111111111111111111111111111111111111",
                "to": null,
                "value": "0x0",
            }),
        },
    ])
    .await;

    let adapter = AlchemyPendingTxStream::new(server.url().clone());
    let mut rx = adapter
        .subscribe(Chain::Ethereum, PendingTxFilter::default())
        .await
        .expect("subscribe ok");

    let first = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("first event arrives within 2s")
        .expect("event");
    let PendingTxEvent::Added(tx) = first else {
        panic!("expected Added, got {first:?}");
    };
    assert_eq!(
        tx.hash,
        TxHash::from_hex("0xaaaa000000000000000000000000000000000000000000000000000000000001")
            .unwrap(),
    );
    assert_eq!(
        tx.from,
        Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
    );
    assert_eq!(
        tx.to,
        Some(Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()),
    );
    assert_eq!(tx.value, Wei::new(1_000));

    let second = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("second event arrives within 2s")
        .expect("event");
    let PendingTxEvent::Added(tx) = second else {
        panic!("expected Added");
    };
    assert_eq!(
        tx.hash,
        TxHash::from_hex("0xaaaa000000000000000000000000000000000000000000000000000000000002")
            .unwrap(),
    );
    assert_eq!(tx.to, None, "null `to` is a contract creation (§11.1)");
    assert_eq!(tx.value, Wei::new(0));

    // Dropping the server here closes the connection; the adapter
    // task exits quietly and drops its events sender. The next recv
    // returns None.
    drop(server);
    let closing = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("channel closes within 2s");
    assert_eq!(closing, None);
}

/// §11.3.4: `update_filter` hand-off from the port to the in-flight
/// task. Today we cannot read the frames the harness received back
/// from the adapter (capture plumbing is a future enhancement — see
/// the TODO in `ws_harness.rs`), so the contract we pin here is
/// narrower: calling `update_filter` before `subscribe` surfaces
/// `DomainError::ProviderUnavailable`, and calling it after a
/// successful subscribe returns `Ok(())`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_filter_before_subscribe_returns_provider_unavailable() {
    let server = CannedWsServer::start(vec![WsStep::ExpectSubscribeReply {
        expected_method: "eth_subscribe",
        sub_id: "0xabc123",
    }])
    .await;
    let adapter = AlchemyPendingTxStream::new(server.url().clone());

    let err = adapter
        .update_filter(Chain::Ethereum, PendingTxFilter::default())
        .await
        .expect_err("no subscribe yet");
    assert!(matches!(
        err,
        blockexplorer_tui::domain::DomainError::ProviderUnavailable,
    ));

    let _rx = adapter
        .subscribe(Chain::Ethereum, PendingTxFilter::default())
        .await
        .expect("subscribe ok");
    adapter
        .update_filter(
            Chain::Ethereum,
            PendingTxFilter {
                from: Some(
                    Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
                ),
            },
        )
        .await
        .expect("update_filter reaches the running task");
}
