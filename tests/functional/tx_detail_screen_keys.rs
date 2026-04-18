//! Key-handling tests for [`TxDetailScreen`].
//!
//! Covers plan/4-tx-detail.md sections 13.1 (Overview row cursor +
//! copy), 13.3 (bounded scrolling) and 13.5 (tab nav with
//! `Shift+Tab` / arrow keys).

use blockexplorer_tui::adapters::ui::{Screen, TxDetailScreen, TxTab, tx_feed};
use blockexplorer_tui::application::TxView;
use blockexplorer_tui::domain::{
    Address, BlockHash, BlockNumber, Chain, Transaction, TxHash, TxStatus, TxType, Wei,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn shift(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::SHIFT)
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
        to: Some(
            Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        ),
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

fn loaded_screen(tx: Transaction) -> TxDetailScreen {
    let (feed, sender) = tx_feed();
    let mut screen = TxDetailScreen::loading(tx.chain, tx.hash, feed);
    sender.updates_tx.send(TxView::bare(tx)).unwrap();
    screen.tick();
    screen
}

#[test]
fn tab_moves_to_the_next_tab() {
    let mut screen = loaded_screen(sample_tx());
    assert_eq!(screen.active_tab(), TxTab::Overview);
    screen.handle_key(key(KeyCode::Tab));
    assert_eq!(screen.active_tab(), TxTab::Logs);
    screen.handle_key(key(KeyCode::Tab));
    assert_eq!(screen.active_tab(), TxTab::AssetChanges);
}

#[test]
fn shift_tab_moves_to_the_previous_tab() {
    let mut screen = loaded_screen(sample_tx());
    assert_eq!(screen.active_tab(), TxTab::Overview);
    screen.handle_key(shift(KeyCode::BackTab));
    assert_eq!(screen.active_tab(), TxTab::Raw);
    screen.handle_key(shift(KeyCode::BackTab));
    assert_eq!(screen.active_tab(), TxTab::StateChanges);
}

#[test]
fn right_and_left_arrows_also_switch_tabs_on_overview() {
    let mut screen = loaded_screen(sample_tx());
    screen.handle_key(key(KeyCode::Right));
    assert_eq!(screen.active_tab(), TxTab::Logs);
    screen.handle_key(key(KeyCode::Left));
    assert_eq!(screen.active_tab(), TxTab::Overview);
    screen.handle_key(key(KeyCode::Left));
    assert_eq!(screen.active_tab(), TxTab::Raw);
}

#[test]
fn up_and_down_cycle_through_overview_rows_without_moving_tabs() {
    let mut screen = loaded_screen(sample_tx());
    assert_eq!(screen.active_tab(), TxTab::Overview);
    let first = screen.overview_selected_row();
    screen.handle_key(key(KeyCode::Down));
    assert_ne!(screen.overview_selected_row(), first);
    assert_eq!(screen.active_tab(), TxTab::Overview);
}

#[test]
fn y_copies_the_selected_overview_row_value() {
    let mut screen = loaded_screen(sample_tx());
    let first_value = screen.overview_selected_copy_value().unwrap();
    screen.handle_key(key(KeyCode::Char('y')));
    assert_eq!(screen.last_copied_value().as_deref(), Some(first_value.as_str()));
}

#[test]
fn y_on_fee_paid_row_copies_raw_wei_amount() {
    let mut screen = loaded_screen(sample_tx());
    while screen.overview_selected_label() != Some("Fee paid") {
        screen.handle_key(key(KeyCode::Down));
    }
    screen.handle_key(key(KeyCode::Char('y')));
    let fee_wei = 14_000_000_000u128 * 52_341u128;
    assert_eq!(
        screen.last_copied_value().as_deref(),
        Some(fee_wei.to_string().as_str())
    );
}
