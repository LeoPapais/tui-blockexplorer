//! Functional tests for [`AlchemyNewHeadsStream`].
//!
//! Drives the adapter against the in-process WS harness in
//! `tests/support/ws_harness.rs`. See
//! `plan/13-alchemy-adapter.md` §8.6 and
//! `plan/15-backlog.md` §8.14 item 1.

use std::sync::Arc;
use std::time::Duration;

use blockexplorer_tui::{
    adapters::rpc::{AlchemyNewHeadsStream, RetryPolicy},
    application::ports::{NewHeadsStreamPort, Rng},
    domain::{BlockNumber, Chain},
};
use pretty_assertions::assert_eq;
use serde_json::json;

use crate::support::stubs::SeededRng;
use crate::support::ws_harness::{CannedWsServer, WsStep};

fn stream_for(url: &url::Url) -> AlchemyNewHeadsStream {
    let rng: Arc<dyn Rng> = Arc::new(SeededRng::new(11));
    AlchemyNewHeadsStream::with_retry_policy(
        url.clone(),
        RetryPolicy::new(
            1, // one connection attempt, no reconnect loop in happy-path tests
            Duration::from_millis(0),
            Duration::from_millis(0),
            rng,
        ),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscribe_translates_new_head_notifications_into_domain_events() {
    let server = CannedWsServer::start(vec![
        WsStep::ExpectSubscribeReply {
            expected_method: "eth_subscribe",
            sub_id: "0xdeadbeef",
        },
        WsStep::EmitNotification {
            sub_id: "0xdeadbeef",
            result: json!({
                "number": "0x10",
                "hash": "0xaaa1000000000000000000000000000000000000000000000000000000000001",
                "parentHash": "0xbbb1000000000000000000000000000000000000000000000000000000000001",
            }),
        },
        WsStep::EmitNotification {
            sub_id: "0xdeadbeef",
            result: json!({
                "number": "0x11",
                "hash": "0xaaa1000000000000000000000000000000000000000000000000000000000002",
                "parentHash": "0xaaa1000000000000000000000000000000000000000000000000000000000001",
            }),
        },
    ])
    .await;

    let adapter = stream_for(server.url());
    let mut rx = adapter
        .subscribe(Chain::Ethereum)
        .await
        .expect("subscribe ok");

    let first = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("first event arrives within 2s")
        .expect("event");
    assert_eq!(first.chain, Chain::Ethereum);
    assert_eq!(first.number, BlockNumber::new(0x10));

    let second = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("second event arrives within 2s")
        .expect("event");
    assert_eq!(second.number, BlockNumber::new(0x11));

    drop(server);
    let closing = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("channel closes within 2s");
    assert_eq!(closing, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_notifications_are_dropped_without_breaking_the_stream() {
    let server = CannedWsServer::start(vec![
        WsStep::ExpectSubscribeReply {
            expected_method: "eth_subscribe",
            sub_id: "0xabc",
        },
        WsStep::EmitNotification {
            sub_id: "0xabc",
            result: json!({
                // Missing `number` key: must be ignored.
                "hash": "0xaaa1000000000000000000000000000000000000000000000000000000000001",
            }),
        },
        WsStep::EmitNotification {
            sub_id: "0xabc",
            result: json!({
                "number": "0x2a",
                "hash": "0xbbb1000000000000000000000000000000000000000000000000000000000002",
                "parentHash": "0xccc1000000000000000000000000000000000000000000000000000000000003",
            }),
        },
    ])
    .await;
    let adapter = stream_for(server.url());
    let mut rx = adapter
        .subscribe(Chain::Polygon)
        .await
        .expect("subscribe ok");

    let head = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("valid notification arrives")
        .expect("event");
    assert_eq!(head.chain, Chain::Polygon);
    assert_eq!(head.number, BlockNumber::new(0x2a));
}
