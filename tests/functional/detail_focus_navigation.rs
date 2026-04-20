//! Hierarchical tab focus (main strip vs body) on detail screens.

use blockexplorer_tui::adapters::ui::{
    AddressDetailScreen, BlockDetailScreen, BlockTab, DetailFocusLayer, Screen, TxDetailScreen,
    address_feed, block_feed, tx_feed,
};
use blockexplorer_tui::application::TxView;
use blockexplorer_tui::domain::{
    Address, AddressKind, AddressOverview, Block, BlockHash, BlockNumber, Chain, Transaction,
    TxHash, TxStatus, TxType, UnixTimestamp, Wei,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn sample_block_with_txs() -> Block {
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
        gas_used: 0,
        gas_limit: 30_000_000,
        base_fee: Some(Wei::new(0)),
        size: 0,
        extra_data: Vec::new(),
        tx_hashes: vec![
            TxHash::from_hex("0x1111111111111111111111111111111111111111111111111111111111111111")
                .unwrap(),
        ],
        extra_signer: None,
        withdrawals: Vec::new(),
    }
}

#[test]
fn block_detail_up_from_first_tx_row_focuses_main_tabs() {
    let (feed, _sender) = block_feed();
    let open_tx = Box::new(|_h| -> Box<dyn Screen> {
        panic!("open_tx should not run");
    });
    let mut screen =
        BlockDetailScreen::with_block(Chain::Ethereum, sample_block_with_txs(), feed, open_tx);
    screen.handle_key(key(KeyCode::Tab));
    assert_eq!(screen.tx_selected(), 0);
    assert_eq!(screen.focus_layer(), DetailFocusLayer::Content);
    screen.handle_key(key(KeyCode::Up));
    assert_eq!(screen.focus_layer(), DetailFocusLayer::MainTabs);
    screen.handle_key(key(KeyCode::Right));
    assert_eq!(screen.active_tab(), BlockTab::BlobsAndWithdrawals);
}

#[test]
fn address_transactions_up_at_top_focuses_main_tabs() {
    let (feed, _sender) = address_feed();
    let addr = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    let mut screen = AddressDetailScreen::loading(Chain::Ethereum, addr, feed);
    screen.set_overview_for_test(AddressOverview {
        chain: Chain::Ethereum,
        address: addr,
        balance: Wei::new(0),
        nonce: 0,
        kind: AddressKind::Eoa { delegated_to: None },
        delegated_to: None,
        ens_name: None,
    });
    screen.handle_key(key(KeyCode::Tab));
    assert_eq!(screen.focus_layer(), DetailFocusLayer::Content);
    screen.handle_key(key(KeyCode::Up));
    assert_eq!(screen.focus_layer(), DetailFocusLayer::MainTabs);
}

#[test]
fn tx_detail_down_from_main_tabs_returns_to_content() {
    let tx = Transaction {
        chain: Chain::Ethereum,
        hash: TxHash::from_hex(
            "0xabcd016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a7139401",
        )
        .unwrap(),
        status: TxStatus::Success,
        block_number: Some(BlockNumber::new(21_345_678)),
        block_hash: None,
        tx_index: Some(0),
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()),
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
    let (feed, sender) = tx_feed();
    let mut screen = TxDetailScreen::loading(tx.chain, tx.hash, feed);
    sender.updates_tx.send(TxView::bare(tx)).unwrap();
    screen.tick();
    screen.handle_key(key(KeyCode::Up));
    assert_eq!(screen.focus_layer(), DetailFocusLayer::MainTabs);
    screen.handle_key(key(KeyCode::Down));
    assert_eq!(screen.focus_layer(), DetailFocusLayer::Content);
}
