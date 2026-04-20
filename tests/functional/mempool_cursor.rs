//! Cursor / clipboard tests for the Mempool screen.
//!
//! See `plan/17-navigable-values.md` §6 (Mempool row) + §8.1.

use std::sync::Arc;

use blockexplorer_tui::{
    adapters::ui::{Command, CursorServices, MempoolScreen, Screen},
    application::ports::ClipboardPort,
    domain::{Address, Chain, PendingTx, PendingTxEvent, PendingTxFilter, TxHash, Wei},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::unbounded_channel;

use super::support::stubs::{StubClipboard, StubNavigationFactory};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn sample_tx() -> PendingTx {
    PendingTx {
        hash: TxHash::from_hex(
            "0x1234123412341234123412341234123412341234123412341234123412341234",
        )
        .unwrap(),
        from: Address::from_hex("0xdddddddddddddddddddddddddddddddddddddddd").unwrap(),
        to: Some(Address::from_hex("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee").unwrap()),
        value: Wei::new(1_000_000_000_000_000_000),
    }
}

fn build_screen_with_event(tx: PendingTx) -> (MempoolScreen, StubClipboard, StubNavigationFactory) {
    let (tx_sender, rx) = unbounded_channel();
    tx_sender
        .send(PendingTxEvent::Added(tx))
        .expect("seed event");
    let open_tx = Box::new(|_hash| -> Box<dyn Screen> { panic!("open_tx should not fire on `y`") });
    let clipboard = StubClipboard::new();
    let nav = StubNavigationFactory::new();
    let services = CursorServices::new(
        Arc::new(clipboard.clone()) as Arc<dyn ClipboardPort>,
        Arc::new(nav.clone()),
        Chain::Ethereum,
    );
    let mut screen =
        MempoolScreen::new(rx, PendingTxFilter::default(), open_tx).with_cursor_services(services);
    // drain the seeded event so the row is rendered.
    screen.tick();
    (screen, clipboard, nav)
}

#[test]
fn y_on_a_row_copies_the_hash_through_the_clipboard_port() {
    let tx = sample_tx();
    let expected = tx.hash.to_hex();
    let (mut screen, clipboard, _) = build_screen_with_event(tx);
    let cmd = screen.handle_key(key(KeyCode::Char('y')));
    assert_eq!(cmd, Command::None);
    assert_eq!(clipboard.last_copied().as_deref(), Some(expected.as_str()));
    assert_eq!(screen.last_copied_value(), Some(expected.as_str()));
}

#[test]
fn y_without_items_is_a_noop() {
    let (tx_sender, rx) = unbounded_channel();
    std::mem::drop(tx_sender); // the channel is live but empty
    let open_tx = Box::new(|_hash| -> Box<dyn Screen> { panic!() });
    let clipboard = StubClipboard::new();
    let services = CursorServices::new(
        Arc::new(clipboard.clone()) as Arc<dyn ClipboardPort>,
        Arc::new(StubNavigationFactory::new()),
        Chain::Ethereum,
    );
    let mut screen =
        MempoolScreen::new(rx, PendingTxFilter::default(), open_tx).with_cursor_services(services);
    screen.handle_key(key(KeyCode::Char('y')));
    assert_eq!(clipboard.last_copied(), None);
}
