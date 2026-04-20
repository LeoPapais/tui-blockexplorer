//! End-to-end wiring test for the live screen stack: when
//! `CursorServices` are plugged into AddressDetail, TxDetail and
//! BlockDetail, pressing `y` in each must land on the clipboard.
//!
//! This is the regression guard for the bug where
//! `live_cursor_services` was only threaded into `HomeScreen` and
//! every other screen ended up with `cursor_services = None`, so the
//! `y` shortcut silently dropped the value. See `plan/17`.

use std::sync::Arc;

use blockexplorer_tui::{
    adapters::ui::{
        AddressDetailScreen, BlockDetailScreen, CursorServices, Screen, TxDetailScreen,
        address_feed, block_feed, tx_feed,
    },
    application::{TxView, ports::ClipboardPort},
    domain::{
        Address, AddressKind, AddressOverview, Block, BlockHash, BlockNumber, Chain, Transaction,
        TxHash, TxStatus, TxType, UnixTimestamp, Wei,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::support::stubs::{StubClipboard, StubNavigationFactory};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn services(clipboard: &StubClipboard) -> CursorServices {
    CursorServices::new(
        Arc::new(clipboard.clone()) as Arc<dyn ClipboardPort>,
        Arc::new(StubNavigationFactory::new()),
        Chain::Ethereum,
    )
}

fn sample_address() -> Address {
    Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap()
}

fn sample_overview() -> AddressOverview {
    AddressOverview {
        chain: Chain::Ethereum,
        address: sample_address(),
        balance: Wei::new(1_000_000_000_000_000_000u128),
        nonce: 1,
        kind: AddressKind::Eoa { delegated_to: None },
        delegated_to: None,
        ens_name: None,
    }
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
        tx_index: Some(0),
        from: sample_address(),
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

fn sample_block() -> Block {
    Block {
        chain: Chain::Ethereum,
        number: BlockNumber::new(21_345_678),
        hash: BlockHash::from_hex(
            "0xaaaa000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap(),
        parent_hash: BlockHash::from_hex(
            "0x0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap(),
        timestamp: UnixTimestamp::from_seconds(1_710_000_000),
        miner: Address::from_hex("0x1111111111111111111111111111111111111111").unwrap(),
        gas_used: 12_000_000,
        gas_limit: 30_000_000,
        base_fee: Some(Wei::new(11_400_000_000)),
        size: 102_400,
        extra_data: vec![],
        tx_hashes: vec![
            TxHash::from_hex("0x1111111111111111111111111111111111111111111111111111111111111111")
                .unwrap(),
        ],
        extra_signer: None,
        withdrawals: Vec::new(),
    }
}

#[test]
fn address_detail_y_routes_to_clipboard_when_services_are_wired() {
    let clipboard = StubClipboard::new();
    let (feed, _sender) = address_feed();
    let ov = sample_overview();
    let expected = ov.address.to_hex();
    let mut screen = AddressDetailScreen::loading(Chain::Ethereum, ov.address, feed)
        .with_cursor_services(services(&clipboard));
    screen.set_overview_for_test(ov);

    screen.handle_key(key(KeyCode::Char('y')));

    assert_eq!(clipboard.last_copied().as_deref(), Some(expected.as_str()));
}

#[test]
fn tx_detail_y_routes_to_clipboard_when_services_are_wired() {
    let clipboard = StubClipboard::new();
    let (feed, sender) = tx_feed();
    let tx = sample_tx();
    let mut screen =
        TxDetailScreen::loading(tx.chain, tx.hash, feed).with_cursor_services(services(&clipboard));
    sender.updates_tx.send(TxView::bare(tx.clone())).unwrap();
    screen.tick();
    let expected = screen.overview_selected_copy_value().unwrap();

    screen.handle_key(key(KeyCode::Char('y')));

    assert_eq!(clipboard.last_copied().as_deref(), Some(expected.as_str()));
}

#[test]
fn block_detail_y_routes_to_clipboard_when_services_are_wired() {
    let clipboard = StubClipboard::new();
    let (feed, _sender) = block_feed();
    let block = sample_block();
    let expected_hash = block.hash.to_hex();
    let open_tx = Box::new(|_hash| -> Box<dyn Screen> {
        panic!("open_tx must not run in clipboard wiring tests")
    });
    let mut screen = BlockDetailScreen::with_block(Chain::Ethereum, block, feed, open_tx)
        .with_cursor_services(services(&clipboard));

    screen.handle_key(key(KeyCode::Char('y')));

    assert_eq!(
        clipboard.last_copied().as_deref(),
        Some(expected_hash.as_str()),
    );
}
