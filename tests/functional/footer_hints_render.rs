//! Footer-hints render tests.
//!
//! Every screen advertises its active keybindings on a 1-row footer
//! drawn by the runtime. Search/Help overlays paint on top as modals
//! and the footer is NOT drawn in that case. See
//! `plan/15-backlog.md` §8.13 and the TUI rules
//! (`.cursor/rules/tui.mdc`).

use blockexplorer_tui::{
    adapters::ui::{
        AddressDetailScreen, AppConfigSnapshot, BlockDetailScreen, HomeScreen, Screen, ScreenStack,
        SettingsScreen, TxDetailScreen, address_feed, block_feed, tx_feed,
    },
    domain::{
        Address, AddressKind, AddressOverview, Block, BlockHash, BlockNumber, Chain, Transaction,
        TxHash, TxStatus, TxType, UnixTimestamp, Wei,
    },
    infra::runtime::draw_screen_with_footer,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

fn render_stack(stack: &ScreenStack, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    let top = stack.top().expect("non-empty stack");
    terminal
        .draw(|frame| draw_screen_with_footer(frame, frame.area(), top, stack))
        .expect("draw");
    terminal.backend().buffer().clone()
}

fn render_screen(screen: Box<dyn Screen>, width: u16, height: u16) -> Buffer {
    let mut stack = ScreenStack::new();
    stack.push(screen);
    render_stack(&stack, width, height)
}

fn row(buffer: &Buffer, y: u16) -> String {
    let mut out = String::new();
    for x in 0..buffer.area.width {
        out.push_str(buffer[(x, y)].symbol());
    }
    out.trim_end().to_string()
}

fn footer_row(buffer: &Buffer) -> String {
    row(buffer, buffer.area.height - 1)
}

// ---------------------------------------------------------------------------
// Home
// ---------------------------------------------------------------------------

#[test]
fn home_footer_advertises_search_help_and_quit() {
    let buffer = render_screen(Box::new(HomeScreen::with_demo_data()), 120, 30);
    let footer = footer_row(&buffer);
    for needle in ["[/]", "Search", "[?]", "Help", "[q]", "Quit"] {
        assert!(
            footer.contains(needle),
            "home footer must contain `{needle}`; got `{footer}`",
        );
    }
}

// ---------------------------------------------------------------------------
// AddressDetail — Overview tab has Tab/Arrows/Enter/y hints.
// ---------------------------------------------------------------------------

fn sample_address() -> Address {
    Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap()
}

fn address_screen() -> AddressDetailScreen {
    let (feed, _sender) = address_feed();
    let mut screen = AddressDetailScreen::loading(Chain::Ethereum, sample_address(), feed);
    screen.set_overview_for_test(AddressOverview {
        chain: Chain::Ethereum,
        address: sample_address(),
        balance: Wei::new(0),
        nonce: 0,
        kind: AddressKind::Eoa { delegated_to: None },
        delegated_to: None,
        ens_name: None,
    });
    screen
}

#[test]
fn address_detail_footer_advertises_tab_cursor_enter_and_copy() {
    let buffer = render_screen(Box::new(address_screen()), 120, 30);
    let footer = footer_row(&buffer);
    for needle in [
        "[Tab]",
        "Tabs",
        "[←/→]",
        "Tab row",
        "[↑/↓]",
        "Focus",
        "[y]",
        "Copy",
        "[Esc]",
    ] {
        assert!(
            footer.contains(needle),
            "address detail footer must contain `{needle}`; got `{footer}`",
        );
    }
}

// ---------------------------------------------------------------------------
// BlockDetail
// ---------------------------------------------------------------------------

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
        gas_used: 0,
        gas_limit: 30_000_000,
        base_fee: Some(Wei::new(0)),
        size: 0,
        extra_data: Vec::new(),
        tx_hashes: Vec::new(),
        extra_signer: None,
        withdrawals: Vec::new(),
    }
}

#[test]
fn block_detail_footer_lists_navigation_and_copy_hints() {
    let (feed, _sender) = block_feed();
    let open_tx = Box::new(|_h| -> Box<dyn Screen> { panic!("open_tx in footer test") });
    let screen = BlockDetailScreen::with_block(Chain::Ethereum, sample_block(), feed, open_tx);

    // Globals (`/`, `?`, `Esc`) prepend the screen hints; keep the
    // terminal wide enough that `[Y]` is not clipped on the last row.
    let buffer = render_screen(Box::new(screen), 220, 30);
    let footer = footer_row(&buffer);
    for needle in [
        "[Tab]",
        "Prev block",
        "Next block",
        "[←/→]",
        "[↑/↓]",
        "[Enter]",
        "[y]",
        "[Y]",
        "[Esc]",
    ] {
        assert!(
            footer.contains(needle),
            "block detail footer must contain `{needle}`; got `{footer}`",
        );
    }
}

// ---------------------------------------------------------------------------
// TxDetail
// ---------------------------------------------------------------------------

fn sample_tx_hash() -> TxHash {
    TxHash::from_hex("0xabcd016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a7139401").unwrap()
}

fn sample_tx() -> Transaction {
    Transaction {
        chain: Chain::Ethereum,
        hash: sample_tx_hash(),
        status: TxStatus::Success,
        block_number: Some(BlockNumber::new(21_345_678)),
        block_hash: None,
        tx_index: Some(0),
        from: sample_address(),
        to: Some(sample_address()),
        value: Wei::new(0),
        gas_price: Wei::new(0),
        gas_used: Some(0),
        gas_limit: 0,
        nonce: 0,
        tx_type: TxType::DynamicFee,
        input: Vec::new(),
        logs: Vec::new(),
        raw_json: "{}".to_string(),
    }
}

#[test]
fn tx_detail_footer_lists_tab_copy_simulate_back() {
    use blockexplorer_tui::application::TxView;
    let (feed, sender) = tx_feed();
    let mut screen = TxDetailScreen::loading(Chain::Ethereum, sample_tx_hash(), feed);
    sender.updates_tx.send(TxView::bare(sample_tx())).unwrap();
    screen.tick();

    let buffer = render_screen(Box::new(screen), 160, 30);
    let footer = footer_row(&buffer);
    for needle in ["[Tab]", "[y]", "[s]", "Re-simulate", "[Esc]"] {
        assert!(
            footer.contains(needle),
            "tx detail footer must contain `{needle}`; got `{footer}`",
        );
    }
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[test]
fn settings_footer_advertises_cursor_copy_palette_and_back() {
    let screen = SettingsScreen::new(AppConfigSnapshot {
        chain: Chain::Ethereum,
        alchemy_key_present: false,
        config_path_hint: None,
    });
    let buffer = render_screen(Box::new(screen), 120, 30);
    let footer = footer_row(&buffer);
    for needle in ["[Arrows]", "[y]", "Palette", "[Esc]"] {
        assert!(
            footer.contains(needle),
            "settings footer must contain `{needle}`; got `{footer}`",
        );
    }
}

// ---------------------------------------------------------------------------
// Layout sanity: one breadcrumb row plus body plus footer; the last
// row is the footer. The screen is responsible for its own content only.
// ---------------------------------------------------------------------------

#[test]
fn footer_is_always_on_the_last_row() {
    let buffer = render_screen(Box::new(HomeScreen::with_demo_data()), 120, 30);
    assert_eq!(buffer.area.height, 30);
    let footer = footer_row(&buffer);
    assert!(
        !footer.is_empty(),
        "footer row must not be blank when the screen advertises hints",
    );
}
