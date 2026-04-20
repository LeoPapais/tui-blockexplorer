//! Field-cursor flow on the Tx Detail screen (Overview tab).
//!
//! The Overview tab reuses the pre-existing per-row cursor; this
//! file asserts that once [`CursorServices`] is wired, `Enter` on
//! the `Hash` / `Block` / `From` / `To` rows navigates via the
//! stubbed `NavigationFactory` and `y` mirrors the string onto the
//! [`StubClipboard`]. See `plan/17-navigable-values.md` §6 and §8.1.

use std::sync::Arc;

use blockexplorer_tui::{
    adapters::ui::{Command, CursorServices, Screen, TxDetailScreen, TxTab, tx_feed},
    application::{TxView, ports::ClipboardPort},
    domain::{
        Address, BlockHash, BlockNumber, Chain, NavigableValue, Transaction, TxHash, TxStatus,
        TxType, Wei,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use crate::support::stubs::{StubClipboard, StubNavigationFactory};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn sample_tx() -> Transaction {
    Transaction {
        chain: Chain::Ethereum,
        hash: TxHash::from_hex(
            "0xabcd016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a7139401",
        )
        .unwrap(),
        status: TxStatus::Success,
        block_number: Some(BlockNumber::new(21_345_678)),
        block_hash: Some(
            BlockHash::from_hex(
                "0x1111111111111111111111111111111111111111111111111111111111111111",
            )
            .unwrap(),
        ),
        tx_index: Some(3),
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()),
        value: Wei::new(0),
        gas_price: Wei::new(14_000_000_000),
        gas_used: Some(52_341),
        gas_limit: 80_000,
        nonce: 42,
        tx_type: TxType::DynamicFee,
        input: vec![],
        logs: vec![],
        raw_json: "{}".to_string(),
    }
}

fn wire_services() -> (
    TxDetailScreen,
    Arc<StubClipboard>,
    Arc<StubNavigationFactory>,
) {
    let tx = sample_tx();
    let (feed, sender) = tx_feed();
    let clipboard = Arc::new(StubClipboard::new());
    let nav = Arc::new(StubNavigationFactory::new());
    let services = CursorServices::new(
        clipboard.clone() as Arc<dyn ClipboardPort>,
        nav.clone(),
        Chain::Ethereum,
    );
    let mut screen =
        TxDetailScreen::loading(tx.chain, tx.hash, feed).with_cursor_services(services);
    sender.updates_tx.send(TxView::bare(tx)).unwrap();
    screen.tick();
    (screen, clipboard, nav)
}

/// Advance the Overview row cursor to the first row whose label
/// equals `target`. Panics if the row does not surface on the view.
fn advance_to_row(screen: &mut TxDetailScreen, target: &str) {
    for _ in 0..32 {
        if screen.overview_selected_label() == Some(target) {
            return;
        }
        screen.handle_key(key(KeyCode::Down));
    }
    panic!("row `{target}` never became selected on the Overview tab");
}

#[test]
fn tx_detail_cursor_enter_on_from_opens_address_detail() {
    let (mut screen, _, nav) = wire_services();
    assert_eq!(screen.active_tab(), TxTab::Overview);

    advance_to_row(&mut screen, "From");
    let cmd = screen.handle_key(key(KeyCode::Enter));

    // The stub factory returns `None` so the screen does not push,
    // but the recording witnesses the call. The live factory maps
    // the same call to `live_address_detail_screen(..., Overview)`.
    assert!(matches!(cmd, Command::None));
    let recorded = nav.recorded_values();
    assert_eq!(recorded.len(), 1);
    assert!(matches!(recorded[0], NavigableValue::Address(_)));
}

#[test]
fn tx_detail_cursor_enter_on_block_opens_block_detail() {
    let (mut screen, _, nav) = wire_services();
    advance_to_row(&mut screen, "Block");
    screen.handle_key(key(KeyCode::Enter));

    let recorded = nav.recorded_values();
    assert_eq!(recorded.len(), 1);
    assert!(matches!(recorded[0], NavigableValue::BlockNumber(_)));
}

#[test]
fn tx_detail_cursor_y_on_hash_row_ships_through_clipboard() {
    let (mut screen, clipboard, _) = wire_services();
    advance_to_row(&mut screen, "Hash");
    let expected = screen
        .overview_selected_copy_value()
        .expect("hash row exposes a copy value");

    screen.handle_key(key(KeyCode::Char('y')));

    assert_eq!(clipboard.last_copied().as_deref(), Some(expected.as_str()));
}
