//! Cursor tests for the Block Detail screen.
//!
//! Asserts on the navigable-field list, y (real clipboard) and
//! Enter (navigation factory recording). See
//! `plan/17-navigable-values.md` §6 + §8.1.

use std::sync::Arc;

use blockexplorer_tui::{
    adapters::ui::{BlockDetailScreen, BlockTab, Command, CursorServices, Screen, block_feed},
    application::ports::ClipboardPort,
    domain::{
        Address, Block, BlockHash, BlockNumber, Chain, NavigableValue, TxHash, UnixTimestamp, Wei,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::support::stubs::{StubClipboard, StubNavigationFactory};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn sample_block() -> Block {
    let hash =
        BlockHash::from_hex("0xaaaa000000000000000000000000000000000000000000000000000000000000")
            .unwrap();
    let parent =
        BlockHash::from_hex("0xbbbb000000000000000000000000000000000000000000000000000000000000")
            .unwrap();
    Block {
        chain: Chain::Ethereum,
        number: BlockNumber::new(21_345_678),
        hash,
        parent_hash: parent,
        timestamp: UnixTimestamp::from_seconds(1_710_000_000),
        miner: Address::from_hex("0x1111111111111111111111111111111111111111").unwrap(),
        gas_used: 12_000_000,
        gas_limit: 30_000_000,
        base_fee: Some(Wei::new(11_400_000_000)),
        size: 102_400,
        extra_data: vec![0x42, 0x42],
        tx_hashes: vec![
            TxHash::from_hex("0x1111111111111111111111111111111111111111111111111111111111111111")
                .unwrap(),
        ],
        extra_signer: None,
        withdrawals: Vec::new(),
    }
}

fn wire_services(block: Block) -> (BlockDetailScreen, StubClipboard, StubNavigationFactory) {
    let (feed, _sender) = block_feed();
    let open_tx = Box::new(|_hash| -> Box<dyn Screen> {
        panic!("open_tx should not be invoked in cursor tests")
    });
    let clipboard = StubClipboard::new();
    let nav = StubNavigationFactory::new();
    let services = CursorServices::new(
        Arc::new(clipboard.clone()) as Arc<dyn ClipboardPort>,
        Arc::new(nav.clone()),
        Chain::Ethereum,
    );
    let screen = BlockDetailScreen::with_block(Chain::Ethereum, block, feed, open_tx)
        .with_cursor_services(services);
    (screen, clipboard, nav)
}

#[test]
fn overview_exposes_block_hash_parent_hash_and_miner() {
    let block = sample_block();
    let (screen, _, _) = wire_services(block.clone());
    let fields = screen.navigable_fields();
    let labels: Vec<&str> = fields.iter().map(|f| f.label).collect();
    assert_eq!(
        labels,
        vec!["block_hash", "parent_hash", "timestamp", "miner",]
    );
}

#[test]
fn cursor_enter_on_parent_hash_opens_block_detail_by_hash() {
    let block = sample_block();
    let parent = block.parent_hash;
    let (mut screen, _, nav) = wire_services(block);

    assert_eq!(screen.active_tab(), BlockTab::Overview);
    screen.handle_key(key(KeyCode::Right));
    screen.handle_key(key(KeyCode::Right));
    assert_eq!(
        screen.cursor().active(),
        Some(1),
        "cursor should land on parent_hash after two Right presses",
    );
    let cmd = screen.handle_key(key(KeyCode::Enter));
    assert_eq!(cmd, Command::None);

    let recorded = nav.recorded_values();
    assert_eq!(recorded, vec![NavigableValue::BlockHash(parent)]);
}

#[test]
fn cursor_y_copies_hash_under_the_cursor() {
    let block = sample_block();
    let block_hash = block.hash;
    let (mut screen, clipboard, _) = wire_services(block);
    screen.handle_key(key(KeyCode::Right));
    assert_eq!(screen.cursor().active(), Some(0));
    screen.handle_key(key(KeyCode::Char('y')));
    assert_eq!(
        clipboard.last_copied().as_deref(),
        Some(block_hash.to_hex().as_str())
    );
}

#[test]
fn backspace_deactivates_cursor_without_popping() {
    let block = sample_block();
    let (mut screen, _, _) = wire_services(block);
    screen.handle_key(key(KeyCode::Right));
    assert!(screen.cursor().is_active());
    let cmd = screen.handle_key(key(KeyCode::Backspace));
    assert_eq!(cmd, Command::None);
    assert!(!screen.cursor().is_active());
}

#[test]
fn esc_pops_screen_even_when_cursor_is_active() {
    let block = sample_block();
    let (mut screen, _, _) = wire_services(block);
    screen.handle_key(key(KeyCode::Right));
    assert!(screen.cursor().is_active());
    let cmd = screen.handle_key(key(KeyCode::Esc));
    assert_eq!(cmd, Command::Pop);
}
