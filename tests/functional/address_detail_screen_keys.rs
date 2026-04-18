//! Key-handling tests for [`AddressDetailScreen`].
//!
//! Covers the April-2026 plan/6 §11 follow-ups promoted from
//! `plan/15-backlog.md` §8.7:
//!
//! - `y` copies the hex address (matching the block detail pattern
//!   from plan/3 §12.1 / §8.4).
//! - `Y` copies the ENS name when the loaded overview has one, and
//!   falls back to the hex address otherwise.
//! - `e` serialises the currently-active tab to CSV into the same
//!   clipboard sink so snapshot-style assertions stay
//!   deterministic.
//!
//! The assertions drive the screen directly through
//! `Screen::handle_key`, mirroring `block_detail_screen_keys.rs`.

use blockexplorer_tui::adapters::ui::{
    AddressDetailScreen, AddressTab, Screen, address_feed,
};
use blockexplorer_tui::domain::{
    Address, AddressKind, AddressOverview, BlockNumber, Chain, PriceLookup, TokenHolding,
    TokenMetadata, TokenPrice, TransferAsset, TransferCategory, TransferEvent, TransferPage,
    TxHash, UnixTimestamp, Wei,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn shift_y() -> KeyEvent {
    // Crossterm emits `Char('Y')` when the user presses Shift+y.
    KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT)
}

fn addr(hex: &str) -> Address {
    Address::from_hex(hex).unwrap()
}

fn overview_with_ens(ens: Option<&str>) -> AddressOverview {
    AddressOverview {
        chain: Chain::Ethereum,
        address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
        balance: Wei::new(523_140_000_000_000_000_000u128),
        nonce: 1_243,
        kind: AddressKind::Eoa { delegated_to: None },
        delegated_to: None,
        ens_name: ens.map(str::to_string),
    }
}

fn build_screen(ov: AddressOverview) -> AddressDetailScreen {
    let (feed, _sender) = address_feed();
    let mut screen =
        AddressDetailScreen::loading(Chain::Ethereum, ov.address, feed);
    screen.set_overview_for_test(ov);
    screen
}

#[test]
fn lowercase_y_copies_the_hex_address() {
    let ov = overview_with_ens(Some("vitalik.eth"));
    let expected = ov.address.to_hex();
    let mut screen = build_screen(ov);

    screen.handle_key(key(KeyCode::Char('y')));

    assert_eq!(screen.last_copied_value(), Some(expected.as_str()));
}

#[test]
fn uppercase_y_copies_the_ens_name_when_present() {
    let ov = overview_with_ens(Some("vitalik.eth"));
    let mut screen = build_screen(ov);

    screen.handle_key(shift_y());

    assert_eq!(screen.last_copied_value(), Some("vitalik.eth"));
}

#[test]
fn uppercase_y_falls_back_to_hex_when_ens_is_missing() {
    let ov = overview_with_ens(None);
    let expected = ov.address.to_hex();
    let mut screen = build_screen(ov);

    screen.handle_key(shift_y());

    assert_eq!(screen.last_copied_value(), Some(expected.as_str()));
}

#[test]
fn y_before_overview_loads_is_a_noop() {
    let (feed, _sender) = address_feed();
    let mut screen = AddressDetailScreen::loading(
        Chain::Ethereum,
        addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
        feed,
    );

    screen.handle_key(key(KeyCode::Char('y')));
    screen.handle_key(shift_y());

    assert_eq!(screen.last_copied_value(), None);
}

// ---------------------------------------------------------------------------
// CSV export on `e`
// ---------------------------------------------------------------------------

fn sample_transfer_page() -> TransferPage {
    TransferPage {
        events: vec![
            TransferEvent {
                chain: Chain::Ethereum,
                block_number: BlockNumber::new(21_000_000),
                tx_hash: TxHash::from_hex(
                    "0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa",
                )
                .unwrap(),
                from: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
                to: Some(addr("0x0000000000000000000000000000000000000099")),
                asset: TransferAsset::Native {
                    symbol: "ETH".into(),
                },
                value: Wei::new(1_000_000_000_000_000_000),
                category: TransferCategory::External,
            },
            TransferEvent {
                chain: Chain::Ethereum,
                block_number: BlockNumber::new(20_999_999),
                tx_hash: TxHash::from_hex(
                    "0xbbbb016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394bb",
                )
                .unwrap(),
                from: addr("0x0000000000000000000000000000000000000099"),
                to: Some(addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045")),
                asset: TransferAsset::Erc20 {
                    contract: addr("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                    symbol: "USDC".into(),
                    decimals: 6,
                },
                value: Wei::new(42_000_000),
                category: TransferCategory::Erc20,
            },
        ],
        next_cursor: None,
    }
}

fn sample_holdings() -> Vec<TokenHolding> {
    vec![
        TokenHolding {
            metadata: TokenMetadata {
                address: addr("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                symbol: "USDC".into(),
                name: "USD Coin".into(),
                decimals: 6,
            },
            balance: Wei::new(1_000_000),
            price: PriceLookup::Available(TokenPrice {
                currency: "usd".into(),
                value: 1.0001,
                as_of: UnixTimestamp::from_seconds(1),
            }),
        },
        TokenHolding {
            metadata: TokenMetadata {
                address: addr("0xe6a537a407488807f0bbeb0038b79004f19dddfb"),
                symbol: "BRLA".into(),
                name: "BRLA Token".into(),
                decimals: 18,
            },
            balance: Wei::new(5_000_000_000_000_000_000),
            price: PriceLookup::Unsupported {
                provider: "alchemy-prices",
            },
        },
    ]
}

#[test]
fn e_on_transactions_exports_csv_with_header_and_rows() {
    let ov = overview_with_ens(None);
    let mut screen = build_screen(ov);
    screen.set_transfers_for_test(sample_transfer_page());
    // Move to the Transactions tab (Overview -> Transactions).
    screen.handle_key(key(KeyCode::Tab));
    assert_eq!(screen.active_tab(), AddressTab::Transactions);

    screen.handle_key(key(KeyCode::Char('e')));

    let csv = screen
        .last_copied_value()
        .expect("CSV blob written to clipboard sink");
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(
        lines[0],
        "block,tx_hash,category,from,to,asset,symbol,decimals,value",
        "header row",
    );
    assert_eq!(lines.len(), 3, "header + 2 rows");
    assert!(lines[1].starts_with("21000000,0xaaaa"));
    assert!(lines[1].contains(",external,"));
    assert!(lines[1].contains(",ETH,"));
    assert!(lines[2].contains(",erc20,"));
    assert!(lines[2].contains(",USDC,"));
    assert!(lines[2].ends_with(",42000000"));
}

#[test]
fn e_on_tokens_exports_csv_with_price_columns() {
    let ov = overview_with_ens(None);
    let mut screen = build_screen(ov);
    screen.set_holdings_for_test(sample_holdings());
    // Overview -> Transactions -> Tokens.
    screen.handle_key(key(KeyCode::Tab));
    screen.handle_key(key(KeyCode::Tab));
    assert_eq!(screen.active_tab(), AddressTab::Tokens);

    screen.handle_key(key(KeyCode::Char('e')));

    let csv = screen
        .last_copied_value()
        .expect("CSV blob written to clipboard sink");
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(
        lines[0],
        "symbol,name,contract,decimals,balance,price_usd,price_status,value_usd",
    );
    assert_eq!(lines.len(), 3);
    // USDC with a real price.
    assert!(lines[1].starts_with("USDC,"));
    assert!(lines[1].contains(",available,"));
    assert!(lines[1].contains(",1.0001,"));
    // BRLA without a price shows the unsupported status and a blank
    // USD column so spreadsheets do not report a zero.
    assert!(lines[2].starts_with("BRLA,"));
    assert!(lines[2].contains(",unsupported:alchemy-prices,"));
    assert!(lines[2].ends_with(","), "trailing blank value_usd column");
}

#[test]
fn e_on_overview_exports_a_minimal_summary_csv() {
    let ov = overview_with_ens(Some("vitalik.eth"));
    let expected_address = ov.address.to_hex();
    let mut screen = build_screen(ov);
    assert_eq!(screen.active_tab(), AddressTab::Overview);

    screen.handle_key(key(KeyCode::Char('e')));

    let csv = screen
        .last_copied_value()
        .expect("CSV blob written to clipboard sink");
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[0], "address,ens,kind,balance,nonce");
    assert_eq!(lines.len(), 2);
    assert!(lines[1].starts_with(&format!("{},vitalik.eth,eoa,", expected_address)));
}

#[test]
fn e_before_overview_loads_is_a_noop() {
    let (feed, _sender) = address_feed();
    let mut screen = AddressDetailScreen::loading(
        Chain::Ethereum,
        addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
        feed,
    );

    screen.handle_key(key(KeyCode::Char('e')));

    assert_eq!(screen.last_copied_value(), None);
}
