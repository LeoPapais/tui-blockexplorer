//! Functional tests for the [`MempoolScreen`] adapter.
//!
//! Covers the §11.3 follow-ups from `plan/5-mempool.md`:
//!
//! * §10 / §11.3.1 — default filter on launch is
//!   [`PendingTxFilter::default`];
//! * §11.3.2 — `set_filter` prunes the list and forwards the new
//!   predicate onto the control channel;
//! * §11.3.3 — `ConnectionStatus` updates delivered on the status
//!   channel flip the badge.

use std::sync::Arc;

use blockexplorer_tui::{
    adapters::ui::{CursorServices, MempoolScreen, Screen},
    application::{ConnectionStatus, ports::ClipboardPort},
    domain::{Address, Chain, PendingTx, PendingTxEvent, PendingTxFilter, TxHash, Wei},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use super::support::stubs::{StubClipboard, StubNavigationFactory};

fn sample_pending(hash_hex: &str, from_hex: &str) -> PendingTx {
    PendingTx {
        hash: TxHash::from_hex(hash_hex).unwrap(),
        from: Address::from_hex(from_hex).unwrap(),
        to: Some(Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()),
        value: Wei::new(1_000),
    }
}

fn open_tx_panic() -> Box<dyn Fn(TxHash) -> Box<dyn Screen> + Send> {
    Box::new(|_| panic!("tx navigation not exercised by these tests"))
}

fn build_screen() -> (MempoolScreen, UnboundedSender<PendingTxEvent>) {
    let (tx, rx) = unbounded_channel();
    let screen = MempoolScreen::new(rx, PendingTxFilter::default(), open_tx_panic());
    (screen, tx)
}

#[test]
fn default_filter_is_empty() {
    let (screen, _tx) = build_screen();

    assert_eq!(screen.filter(), &PendingTxFilter::default());
    assert_eq!(screen.filter().from, None);
    assert_eq!(
        screen.stream_state(),
        &ConnectionStatus::Connected,
        "a fresh screen always boots in Connected state",
    );
}

#[test]
fn set_filter_forwards_to_control_channel() {
    let (events_tx, events_rx) = unbounded_channel();
    let (control_tx, mut control_rx) = unbounded_channel();
    let mut screen = MempoolScreen::new(events_rx, PendingTxFilter::default(), open_tx_panic())
        .with_filter_control(control_tx);

    let wanted = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    let unwanted = Address::from_hex("0x1111111111111111111111111111111111111111").unwrap();

    events_tx
        .send(PendingTxEvent::Added(sample_pending(
            "0xaaaa000000000000000000000000000000000000000000000000000000000001",
            "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
        )))
        .unwrap();
    events_tx
        .send(PendingTxEvent::Added(sample_pending(
            "0xaaaa000000000000000000000000000000000000000000000000000000000002",
            "0x1111111111111111111111111111111111111111",
        )))
        .unwrap();
    screen.tick();
    assert_eq!(screen.items().len(), 2);

    let new_filter = PendingTxFilter { from: Some(wanted) };
    screen.set_filter(new_filter);

    assert_eq!(screen.items().len(), 1, "prune semantic from §11.3.2");
    assert_eq!(screen.items()[0].from, wanted);
    assert_eq!(screen.filter().from, Some(wanted));

    let broadcast = control_rx.try_recv().expect("filter broadcast");
    assert_eq!(broadcast, new_filter);

    let second = PendingTxFilter {
        from: Some(unwanted),
    };
    screen.set_filter(second);
    assert_eq!(control_rx.try_recv().expect("second broadcast"), second);
    assert_eq!(screen.items().len(), 0, "prune after second filter change");
}

#[test]
fn set_filter_without_control_is_still_client_side() {
    let (mut screen, events_tx) = build_screen();
    let wanted = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

    events_tx
        .send(PendingTxEvent::Added(sample_pending(
            "0xaaaa000000000000000000000000000000000000000000000000000000000001",
            "0x1111111111111111111111111111111111111111",
        )))
        .unwrap();
    screen.tick();
    assert_eq!(screen.items().len(), 1);

    screen.set_filter(PendingTxFilter { from: Some(wanted) });
    assert_eq!(
        screen.items().len(),
        0,
        "client-side prune happens even without a control channel"
    );
}

#[test]
fn status_feed_flips_badge_to_reconnecting() {
    let (events_tx, events_rx) = unbounded_channel();
    let (status_tx, status_rx) = unbounded_channel();
    let mut screen = MempoolScreen::new(events_rx, PendingTxFilter::default(), open_tx_panic())
        .with_status_feed(status_rx);

    events_tx
        .send(PendingTxEvent::Added(sample_pending(
            "0xaaaa000000000000000000000000000000000000000000000000000000000001",
            "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
        )))
        .unwrap();
    screen.tick();
    assert_eq!(screen.items().len(), 1);
    assert_eq!(screen.stream_state(), &ConnectionStatus::Connected);

    status_tx
        .send(ConnectionStatus::Disconnected {
            reconnect_scheduled: true,
        })
        .unwrap();
    screen.tick();
    assert_eq!(
        screen.stream_state(),
        &ConnectionStatus::Disconnected {
            reconnect_scheduled: true
        },
    );
    assert_eq!(
        screen.items().len(),
        1,
        "buffered events survive a reconnecting state (§11.3.3)",
    );

    status_tx.send(ConnectionStatus::Connected).unwrap();
    screen.tick();
    assert_eq!(screen.stream_state(), &ConnectionStatus::Connected);
}

#[test]
fn y_without_cursor_still_routes_to_clipboard_when_services_are_present() {
    let (events_tx, events_rx) = unbounded_channel();
    let clipboard = StubClipboard::new();
    let services = CursorServices::new(
        Arc::new(clipboard.clone()) as Arc<dyn ClipboardPort>,
        Arc::new(StubNavigationFactory::new()),
        Chain::Ethereum,
    );
    let mut screen = MempoolScreen::new(events_rx, PendingTxFilter::default(), open_tx_panic())
        .with_cursor_services(services);

    let hash =
        TxHash::from_hex("0xaaaa000000000000000000000000000000000000000000000000000000000001")
            .unwrap();
    events_tx
        .send(PendingTxEvent::Added(PendingTx {
            hash,
            from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
            to: None,
            value: Wei::new(0),
        }))
        .unwrap();
    screen.tick();

    screen.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

    assert_eq!(
        clipboard.last_copied().as_deref(),
        Some(hash.to_hex().as_str()),
    );
}

#[test]
fn events_channel_close_auto_flips_to_reconnecting() {
    let (events_tx, events_rx) = unbounded_channel();
    let mut screen = MempoolScreen::new(events_rx, PendingTxFilter::default(), open_tx_panic());

    drop(events_tx);
    screen.tick();

    assert_eq!(
        screen.stream_state(),
        &ConnectionStatus::Disconnected {
            reconnect_scheduled: true
        },
    );
}
