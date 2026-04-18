//! Key-handling tests for [`BlockDetailScreen`].
//!
//! Covers `plan/3-block-detail.md` §12.1 (clipboard `y` / `Y`
//! bindings) and §12.2 (category badges on the Transactions tab).
//! The tests drive the screen directly through `Screen::handle_key`
//! so the assertions do not depend on any terminal wiring.

use blockexplorer_tui::adapters::ui::{BlockDetailScreen, BlockTab, Screen, block_feed};
use blockexplorer_tui::domain::{
    Address, Block, BlockHash, BlockNumber, Chain, TxHash, UnixTimestamp, Wei,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn shift(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::SHIFT)
}

fn sample_block() -> Block {
    let hash =
        BlockHash::from_hex("0xaaaa000000000000000000000000000000000000000000000000000000000000")
            .unwrap();
    let parent =
        BlockHash::from_hex("0x0000000000000000000000000000000000000000000000000000000000000000")
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
            TxHash::from_hex("0x2222222222222222222222222222222222222222222222222222222222222222")
                .unwrap(),
        ],
        extra_signer: None,
    }
}

fn build_screen(block: Block) -> BlockDetailScreen {
    let (feed, _sender) = block_feed();
    let open_tx = Box::new(|_hash| -> Box<dyn Screen> {
        panic!("open_tx should not be invoked in clipboard tests");
    });
    BlockDetailScreen::with_block(Chain::Ethereum, block, feed, open_tx)
}

#[test]
fn y_on_overview_copies_the_block_hash() {
    let block = sample_block();
    let expected_hash = block.hash.to_hex();
    let mut screen = build_screen(block);

    assert_eq!(screen.active_tab(), BlockTab::Overview);

    screen.handle_key(key(KeyCode::Char('y')));

    assert_eq!(screen.last_copied_value(), Some(expected_hash.as_str()));
}

#[test]
fn y_on_transactions_copies_the_selected_tx_hash() {
    let block = sample_block();
    let expected_first = block.tx_hashes[0].to_hex();
    let expected_second = block.tx_hashes[1].to_hex();
    let mut screen = build_screen(block);

    screen.handle_key(key(KeyCode::Tab));
    assert_eq!(screen.active_tab(), BlockTab::Transactions);

    screen.handle_key(key(KeyCode::Char('y')));
    assert_eq!(screen.last_copied_value(), Some(expected_first.as_str()));

    screen.handle_key(key(KeyCode::Down));
    screen.handle_key(key(KeyCode::Char('y')));
    assert_eq!(screen.last_copied_value(), Some(expected_second.as_str()));
}

#[test]
fn uppercase_y_copies_the_block_number() {
    let block = sample_block();
    let expected_number = block.number.value().to_string();
    let mut screen = build_screen(block);

    screen.handle_key(shift(KeyCode::Char('Y')));

    assert_eq!(screen.last_copied_value(), Some(expected_number.as_str()));
}

#[test]
fn uppercase_y_still_works_from_the_transactions_tab() {
    let block = sample_block();
    let expected_number = block.number.value().to_string();
    let mut screen = build_screen(block);

    screen.handle_key(key(KeyCode::Tab));
    assert_eq!(screen.active_tab(), BlockTab::Transactions);

    screen.handle_key(shift(KeyCode::Char('Y')));
    assert_eq!(screen.last_copied_value(), Some(expected_number.as_str()));
}

#[test]
fn y_before_the_block_is_loaded_is_a_no_op() {
    let (feed, _sender) = block_feed();
    let open_tx = Box::new(|_hash| -> Box<dyn Screen> {
        panic!("open_tx should not be invoked in clipboard tests");
    });
    let mut screen = BlockDetailScreen::loading(
        Chain::Ethereum,
        blockexplorer_tui::domain::BlockId::Number(BlockNumber::new(42)),
        feed,
        open_tx,
    );

    screen.handle_key(key(KeyCode::Char('y')));
    assert_eq!(screen.last_copied_value(), None);

    screen.handle_key(shift(KeyCode::Char('Y')));
    assert_eq!(screen.last_copied_value(), None);
}
