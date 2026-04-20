//! Cursor tests for the unified Address Detail screen (Overview tab).
//!
//! Covers `plan/17-navigable-values.md` §6 (AddressDetail row) and
//! §8.1 (per-screen cursor test layout).

use std::sync::Arc;

use blockexplorer_tui::{
    adapters::ui::{
        AddressDetailScreen, AddressTab, Command, CursorServices, Screen, address_feed,
    },
    application::ports::ClipboardPort,
    domain::{Address, AddressKind, AddressOverview, Chain, NavigableValue, Wei},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::support::stubs::{StubClipboard, StubNavigationFactory};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn addr(hex: &str) -> Address {
    Address::from_hex(hex).unwrap()
}

fn overview_with_ens(ens: Option<&str>) -> AddressOverview {
    AddressOverview {
        chain: Chain::Ethereum,
        address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
        balance: Wei::new(1_000_000_000_000_000_000u128),
        nonce: 1,
        kind: AddressKind::Eoa { delegated_to: None },
        delegated_to: None,
        ens_name: ens.map(str::to_string),
    }
}

fn wire_services(
    ov: AddressOverview,
) -> (AddressDetailScreen, StubClipboard, StubNavigationFactory) {
    let (feed, _sender) = address_feed();
    let clipboard = StubClipboard::new();
    let nav = StubNavigationFactory::new();
    let services = CursorServices::new(
        Arc::new(clipboard.clone()) as Arc<dyn ClipboardPort>,
        Arc::new(nav.clone()),
        Chain::Ethereum,
    );
    let mut screen = AddressDetailScreen::loading(Chain::Ethereum, ov.address, feed)
        .with_cursor_services(services);
    screen.set_overview_for_test(ov);
    (screen, clipboard, nav)
}

#[test]
fn overview_exposes_address_and_ens_when_present() {
    let ov = overview_with_ens(Some("vitalik.eth"));
    let address = ov.address;
    let (screen, _, _) = wire_services(ov);
    assert_eq!(screen.active_tab(), AddressTab::Overview);
    let fields = screen.navigable_fields();
    let labels: Vec<&str> = fields.iter().map(|f| f.label).collect();
    assert_eq!(labels, vec!["address", "ens"]);
    assert_eq!(fields[0].value, NavigableValue::Address(address));
    assert_eq!(
        fields[1].value,
        NavigableValue::EnsName("vitalik.eth".to_string()),
    );
}

#[test]
fn cursor_y_on_address_copies_hex_through_clipboard() {
    let ov = overview_with_ens(None);
    let expected = ov.address.to_hex();
    let (mut screen, clipboard, _) = wire_services(ov);
    screen.handle_key(key(KeyCode::Right));
    assert!(screen.cursor().is_active());
    screen.handle_key(key(KeyCode::Char('y')));
    assert_eq!(clipboard.last_copied().as_deref(), Some(expected.as_str()));
}

#[test]
fn cursor_enter_on_address_opens_address_detail_via_factory() {
    let ov = overview_with_ens(None);
    let expected = ov.address;
    let (mut screen, _, nav) = wire_services(ov);
    screen.handle_key(key(KeyCode::Right));
    let cmd = screen.handle_key(key(KeyCode::Enter));
    assert_eq!(cmd, Command::None);
    let recorded = nav.recorded_values();
    assert_eq!(recorded, vec![NavigableValue::Address(expected)]);
}

#[test]
fn cursor_backspace_deactivates_without_popping() {
    let ov = overview_with_ens(None);
    let (mut screen, _, _) = wire_services(ov);
    screen.handle_key(key(KeyCode::Right));
    assert!(screen.cursor().is_active());
    assert_eq!(screen.handle_key(key(KeyCode::Backspace)), Command::None);
    assert!(!screen.cursor().is_active());
}

#[test]
fn esc_pops_screen_even_when_cursor_is_active() {
    let ov = overview_with_ens(None);
    let (mut screen, _, _) = wire_services(ov);
    screen.handle_key(key(KeyCode::Right));
    assert!(screen.cursor().is_active());
    assert_eq!(screen.handle_key(key(KeyCode::Esc)), Command::Pop);
}
