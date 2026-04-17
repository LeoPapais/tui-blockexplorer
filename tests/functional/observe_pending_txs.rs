//! Functional tests for `observe_pending_txs`.
//!
//! See `plan/5-mempool.md` section 11.1.

use blockexplorer_tui::{
    application::use_cases::observe_pending_txs,
    domain::{
        Address, Chain, PendingTx, PendingTxEvent, PendingTxFilter, TxHash, Wei,
    },
};
use pretty_assertions::assert_eq;

use crate::support::stubs::StubPendingTxStreamPort;

fn sample_pending(hash_hex: &str) -> PendingTx {
    PendingTx {
        hash: TxHash::from_hex(hash_hex).unwrap(),
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(
            Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        ),
        value: Wei::new(1_000),
    }
}

#[tokio::test]
async fn events_arrive_in_insertion_order() {
    let port = StubPendingTxStreamPort::new();
    let mut rx = observe_pending_txs::run(&port, Chain::Ethereum, PendingTxFilter::default())
        .await
        .expect("subscribe ok");

    let a = sample_pending(
        "0xaaaa000000000000000000000000000000000000000000000000000000000001",
    );
    let b = sample_pending(
        "0xaaaa000000000000000000000000000000000000000000000000000000000002",
    );
    port.push_added(a.clone());
    port.push_added(b.clone());

    assert_eq!(rx.recv().await, Some(PendingTxEvent::Added(a)));
    assert_eq!(rx.recv().await, Some(PendingTxEvent::Added(b)));
}

#[tokio::test]
async fn removed_events_are_forwarded() {
    let port = StubPendingTxStreamPort::new();
    let mut rx = observe_pending_txs::run(&port, Chain::Ethereum, PendingTxFilter::default())
        .await
        .expect("subscribe ok");

    let hash = TxHash::from_hex(
        "0xaaaa000000000000000000000000000000000000000000000000000000000003",
    )
    .unwrap();
    port.push_removed(hash);

    assert_eq!(rx.recv().await, Some(PendingTxEvent::Removed(hash)));
}

#[tokio::test]
async fn each_subscribe_call_gets_its_own_channel() {
    let port = StubPendingTxStreamPort::new();
    let mut a = observe_pending_txs::run(&port, Chain::Ethereum, PendingTxFilter::default())
        .await
        .unwrap();
    let mut b = observe_pending_txs::run(&port, Chain::Ethereum, PendingTxFilter::default())
        .await
        .unwrap();

    let tx = sample_pending(
        "0xaaaa000000000000000000000000000000000000000000000000000000000004",
    );
    port.push_added(tx.clone());

    assert_eq!(a.recv().await, Some(PendingTxEvent::Added(tx.clone())));
    assert_eq!(b.recv().await, Some(PendingTxEvent::Added(tx)));
}

#[tokio::test]
async fn filter_matches_rejects_different_sender() {
    let port = StubPendingTxStreamPort::new();
    let wanted = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    let unwanted = Address::from_hex("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();

    // The filter is applied client-side on the screen, but the
    // predicate itself lives in the domain type. Verifying it here
    // keeps the screen's filter logic honest.
    let filter = PendingTxFilter { from: Some(wanted) };
    let mut matching = sample_pending(
        "0xaaaa000000000000000000000000000000000000000000000000000000000005",
    );
    matching.from = wanted;
    let mut dropped = sample_pending(
        "0xaaaa000000000000000000000000000000000000000000000000000000000006",
    );
    dropped.from = unwanted;

    assert!(filter.matches(&matching));
    assert!(!filter.matches(&dropped));

    // Port itself does not filter in MVP — both events flow through
    // the channel and the screen is expected to skip the rejected one.
    let mut rx = observe_pending_txs::run(&port, Chain::Ethereum, filter)
        .await
        .unwrap();
    port.push_added(matching.clone());
    port.push_added(dropped.clone());

    assert_eq!(rx.recv().await, Some(PendingTxEvent::Added(matching)));
    assert_eq!(rx.recv().await, Some(PendingTxEvent::Added(dropped)));
}
