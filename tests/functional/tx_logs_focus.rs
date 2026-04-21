//! Logs tab nested focus (list / decoded / raw).

use std::sync::Arc;

use blockexplorer_tui::{
    adapters::ui::{CursorServices, Screen, TxDetailScreen, TxLogsPane, TxTab, tx_feed},
    application::{
        DecodedLog, DecodedSignature, LoadStatus, SignatureSource, TxView, ports::ClipboardPort,
    },
    domain::{Address, Chain, LogEntry, Transaction, TxHash, TxStatus, TxType, Wei},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use crate::support::stubs::{StubClipboard, StubNavigationFactory};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn sample_log_view() -> TxView {
    let addr = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    let tx = Transaction {
        chain: Chain::Ethereum,
        hash: TxHash::from_hex(
            "0xabcd016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a7139401",
        )
        .unwrap(),
        status: TxStatus::Success,
        block_number: Some(blockexplorer_tui::domain::BlockNumber::new(21_345_678)),
        block_hash: None,
        tx_index: Some(0),
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(addr),
        value: Wei::new(0),
        gas_price: Wei::new(0),
        gas_used: Some(0),
        gas_limit: 0,
        nonce: 0,
        tx_type: TxType::DynamicFee,
        input: Vec::new(),
        logs: Vec::new(),
        raw_json: "{}".to_string(),
    };
    let raw = LogEntry {
        address: addr,
        topics: vec![[1u8; 32], [2u8; 32]],
        data: vec![3, 4],
    };
    let decoded = DecodedLog {
        raw,
        signature: Some(DecodedSignature {
            signature: "Transfer(address,address,uint256)".to_string(),
            source: SignatureSource::Openchain,
            parsed: None,
        }),
    };
    TxView {
        tx,
        decoded_logs: vec![decoded],
        decoded_method: None,
        call_tree: LoadStatus::Unsupported,
        asset_changes: LoadStatus::Unsupported,
        state_diff: LoadStatus::Unsupported,
    }
}

#[test]
fn logs_tab_moves_list_detail_raw_and_esc_peels() {
    let (feed, sender) = tx_feed();
    let mut screen = TxDetailScreen::loading(Chain::Ethereum, sample_log_view().tx.hash, feed);
    sender.updates_tx.send(sample_log_view()).unwrap();
    screen.tick();
    assert_eq!(screen.active_tab(), TxTab::Overview);
    screen.handle_key(key(KeyCode::Tab));
    assert_eq!(screen.active_tab(), TxTab::Logs);
    assert_eq!(screen.logs_tab_pane(), Some(TxLogsPane::List));
    screen.handle_key(key(KeyCode::Right));
    assert_eq!(screen.logs_tab_pane(), Some(TxLogsPane::Decoded));
    screen.handle_key(key(KeyCode::Tab));
    assert_eq!(screen.logs_tab_pane(), Some(TxLogsPane::Raw));
    screen.handle_key(key(KeyCode::Esc));
    assert_eq!(screen.logs_tab_pane(), Some(TxLogsPane::Decoded));
    screen.handle_key(key(KeyCode::Esc));
    assert_eq!(screen.logs_tab_pane(), Some(TxLogsPane::List));
}

#[test]
fn logs_raw_pane_copies_selected_line_with_y() {
    let (feed, sender) = tx_feed();
    let clipboard = Arc::new(StubClipboard::new());
    let nav = Arc::new(StubNavigationFactory::new());
    let services = CursorServices::new(
        clipboard.clone() as Arc<dyn ClipboardPort>,
        nav,
        Chain::Ethereum,
    );
    let mut screen = TxDetailScreen::loading(Chain::Ethereum, sample_log_view().tx.hash, feed)
        .with_cursor_services(services);
    sender.updates_tx.send(sample_log_view()).unwrap();
    screen.tick();
    screen.handle_key(key(KeyCode::Tab));
    assert_eq!(screen.active_tab(), TxTab::Logs);
    screen.handle_key(key(KeyCode::Right));
    screen.handle_key(key(KeyCode::Tab));
    assert_eq!(screen.logs_tab_pane(), Some(TxLogsPane::Raw));
    screen.handle_key(key(KeyCode::Down));
    screen.handle_key(key(KeyCode::Char('y')));
    let copied = screen.last_copied_value().expect("copy");
    assert!(
        !copied.contains("topic0:"),
        "raw log copy must be value-only (plan/18 Slice F), got {copied:?}"
    );
    assert!(
        copied.starts_with("0x"),
        "expected hex payload after label strip, got {copied:?}"
    );
    assert_eq!(
        clipboard.last_copied().as_deref(),
        Some(copied.as_str()),
        "clipboard sink must match last_copied_value"
    );
}
