//! Key-handling tests for [`TxDetailScreen`].
//!
//! Covers plan/4-tx-detail.md sections 13.1 (Overview row cursor +
//! copy), 13.3 (bounded scrolling), 13.5 (tab nav with `Shift+Tab`
//! / arrow keys) and 12.6.3 (`s` re-simulate on pending txs).

use std::sync::Arc;

use blockexplorer_tui::adapters::ui::{
    CursorServices, DetailFocusLayer, Screen, TxDetailScreen, TxTab, tx_feed,
};
use blockexplorer_tui::application::ports::ClipboardPort;
use blockexplorer_tui::application::{LoadStatus, TxView};
use blockexplorer_tui::domain::{
    Address, BlockHash, BlockNumber, CallKind, CallNode, Chain, Transaction, TxHash, TxStatus,
    TxType, Wei,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

use super::support::stubs::{StubClipboard, StubNavigationFactory};

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
    // Plan 12.6.5: Internal tab ships between Logs and Asset Changes.
    assert_eq!(screen.active_tab(), TxTab::Internal);
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
fn left_right_switch_tabs_only_when_tab_strip_is_focused() {
    let mut screen = loaded_screen(sample_tx());
    assert_eq!(screen.focus_layer(), DetailFocusLayer::Content);
    screen.handle_key(key(KeyCode::Up));
    assert_eq!(screen.focus_layer(), DetailFocusLayer::MainTabs);
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
    assert_eq!(
        screen.last_copied_value().as_deref(),
        Some(first_value.as_str())
    );
}

#[test]
fn y_without_cursor_still_routes_to_clipboard_when_services_are_present() {
    let mut screen = loaded_screen(sample_tx());
    let clipboard = StubClipboard::new();
    let services = CursorServices::new(
        Arc::new(clipboard.clone()) as Arc<dyn ClipboardPort>,
        Arc::new(StubNavigationFactory::new()),
        Chain::Ethereum,
    );
    screen = screen.with_cursor_services(services);

    let expected = screen.overview_selected_copy_value().unwrap();
    screen.handle_key(key(KeyCode::Char('y')));

    assert_eq!(clipboard.last_copied().as_deref(), Some(expected.as_str()));
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

/// `s` on a mined tx is a no-op (plan 12.6.3).
#[test]
fn s_is_noop_when_tx_is_mined() {
    let mut screen = loaded_screen(sample_tx());
    assert_eq!(screen.resimulate_count(), 0);
    screen.handle_key(key(KeyCode::Char('s')));
    assert_eq!(screen.resimulate_count(), 0);
}

/// `s` on a pending tx resends the hash on the feed and resets the
/// Asset Changes / State Changes enrichment statuses to `Pending`
/// so the tabs render their loading text until the background task
/// replies.
#[test]
fn s_on_pending_tx_triggers_resimulate() {
    let mut tx = sample_tx();
    tx.status = TxStatus::Pending;
    tx.block_number = None;
    tx.block_hash = None;
    tx.tx_index = None;
    tx.gas_used = None;

    let (feed, sender) = tx_feed();
    let blockexplorer_tui::adapters::ui::TxFeedSender {
        updates_tx,
        mut input_rx,
    } = sender;

    let mut screen = TxDetailScreen::loading(tx.chain, tx.hash, feed);
    // Initial load request.
    assert_eq!(input_rx.try_recv().ok(), Some(tx.hash));

    // Deliver a pending TxView with loaded enrichment slots so we
    // can assert the `s` binding flips them back to `Pending`.
    let mut view = TxView::bare(tx.clone());
    view.asset_changes = LoadStatus::Loaded(Vec::new());
    updates_tx.send(view).unwrap();
    screen.tick();

    assert_eq!(screen.resimulate_count(), 0);
    screen.handle_key(key(KeyCode::Char('s')));
    assert_eq!(screen.resimulate_count(), 1);

    // The resimulate binding resends the hash on the input channel.
    assert_eq!(input_rx.try_recv().ok(), Some(tx.hash));
    // And resets enrichment statuses.
    let current = screen.current().expect("view loaded");
    assert!(matches!(current.asset_changes, LoadStatus::Pending));
    assert!(matches!(current.state_diff, LoadStatus::Pending));
}

fn render_to_buffer(screen: &TxDetailScreen) -> Buffer {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| screen.render(frame, frame.area()))
        .expect("draw");
    terminal.backend().buffer().clone()
}

fn buffer_contains(buffer: &Buffer, needle: &str) -> bool {
    for y in 0..buffer.area.height {
        let mut row = String::new();
        for x in 0..buffer.area.width {
            row.push_str(buffer[(x, y)].symbol());
        }
        if row.contains(needle) {
            return true;
        }
    }
    false
}

/// Snapshot test for the Internal tab: once the tracer delivers a
/// call tree, the rendered frame shows the root `CALL` and the
/// nested `STATICCALL` child with the ASCII elbow.
/// See `plan/4-tx-detail.md` section 12.6.5.
#[test]
fn internal_tab_renders_call_tree_with_elbows() {
    let tx = sample_tx();
    let (feed, sender) = tx_feed();
    let mut screen = TxDetailScreen::loading(tx.chain, tx.hash, feed);
    let child_to = Address::from_hex("0x1111111111111111111111111111111111111111").unwrap();
    let tree = CallNode {
        kind: CallKind::Call,
        from: tx.from,
        to: tx.to,
        value: Wei::new(0),
        input: Vec::new(),
        output: Vec::new(),
        gas_used: 52_341,
        error: None,
        children: vec![CallNode {
            kind: CallKind::Staticcall,
            from: tx.to.unwrap(),
            to: Some(child_to),
            value: Wei::new(0),
            input: Vec::new(),
            output: Vec::new(),
            gas_used: 128,
            error: None,
            children: Vec::new(),
        }],
    };
    let mut view = TxView::bare(tx);
    view.call_tree = LoadStatus::Loaded(tree);
    sender.updates_tx.send(view).unwrap();
    screen.tick();
    // Overview → Logs → Internal.
    screen.handle_key(key(KeyCode::Tab));
    screen.handle_key(key(KeyCode::Tab));
    assert_eq!(screen.active_tab(), TxTab::Internal);

    let buffer = render_to_buffer(&screen);
    assert!(
        buffer_contains(&buffer, "CALL ->"),
        "root CALL frame should render"
    );
    assert!(
        buffer_contains(&buffer, "`- STATICCALL ->"),
        "staticcall child should render with ASCII elbow"
    );
}
