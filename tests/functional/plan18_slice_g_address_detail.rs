//! plan/18 slice G — token overview navigable fields and contract ABI `y` copy.

use std::sync::Arc;

use blockexplorer_tui::{
    adapters::ui::{
        AddressDetailScreen, AddressTab, ContractSubTab, CursorServices, Screen, address_feed,
    },
    application::ports::ClipboardPort,
    domain::{
        Address, AddressKind, AddressOverview, Chain, ContractOverview, ContractSource,
        NavigableValue, SourceFile, TokenMetadata, TokenOverview, Wei,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::support::stubs::{StubClipboard, StubNavigationFactory};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn usdc() -> Address {
    Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
}

fn contract_overview() -> AddressOverview {
    AddressOverview {
        chain: Chain::Ethereum,
        address: usdc(),
        balance: Wei::new(0),
        nonce: 1,
        kind: AddressKind::Contract,
        delegated_to: None,
        ens_name: None,
    }
}

fn token_overview() -> TokenOverview {
    TokenOverview {
        metadata: TokenMetadata {
            address: usdc(),
            symbol: "USDC".into(),
            name: "USD Coin".into(),
            decimals: 6,
        },
        total_supply: 35_000_000_000_000,
        price: blockexplorer_tui::domain::PriceLookup::Pending,
    }
}

fn minimal_abi_source() -> ContractSource {
    ContractSource {
        is_verified: true,
        contract_name: "T".into(),
        compiler_version: "0.8".into(),
        optimizer_enabled: false,
        optimizer_runs: 0,
        evm_version: "paris".into(),
        license: "MIT".into(),
        abi: r#"[{"type":"function","name":"symbol","inputs":[],"outputs":[{"internalType":"string","name":"","type":"string"}],"stateMutability":"view"}]"#
            .into(),
        files: vec![SourceFile {
            path: "T.sol".into(),
            content: "contract T {}".into(),
        }],
        implementation: None,
    }
}

#[test]
fn token_overview_navigable_fields_include_token_address() {
    let (feed, _sender) = address_feed();
    let mut screen = AddressDetailScreen::with_factories_and_tab(
        Chain::Ethereum,
        usdc(),
        feed,
        None,
        None,
        AddressTab::Token,
    );
    screen.set_overview_for_test(contract_overview());
    screen.set_token_overview_for_test(Some(token_overview()));
    let fields = screen.navigable_fields();
    let labels: Vec<&str> = fields.iter().map(|f| f.label).collect();
    assert!(labels.contains(&"address"));
    assert_eq!(
        fields.iter().find(|f| f.label == "address").unwrap().value,
        NavigableValue::TokenAddress(usdc())
    );
}

#[test]
fn token_overview_cursor_y_copies_symbol_plain_text() {
    let (feed, _sender) = address_feed();
    let clipboard = StubClipboard::new();
    let nav = StubNavigationFactory::new();
    let services = CursorServices::new(
        Arc::new(clipboard.clone()) as Arc<dyn ClipboardPort>,
        Arc::new(nav),
        Chain::Ethereum,
    );
    let mut screen = AddressDetailScreen::with_factories_and_tab(
        Chain::Ethereum,
        usdc(),
        feed,
        None,
        None,
        AddressTab::Token,
    )
    .with_cursor_services(services);
    screen.set_overview_for_test(contract_overview());
    screen.set_token_overview_for_test(Some(token_overview()));
    screen.handle_key(key(KeyCode::Right));
    screen.handle_key(key(KeyCode::Right));
    assert!(screen.cursor().is_active());
    assert_eq!(screen.cursor().active(), Some(1));
    screen.handle_key(key(KeyCode::Char('y')));
    assert_eq!(clipboard.last_copied().as_deref(), Some("USDC"));
}

#[test]
fn y_on_abi_subtab_copies_full_abi_json() {
    let (feed, _sender) = address_feed();
    let clipboard = StubClipboard::new();
    let nav = StubNavigationFactory::new();
    let services = CursorServices::new(
        Arc::new(clipboard.clone()) as Arc<dyn ClipboardPort>,
        Arc::new(nav),
        Chain::Ethereum,
    );
    let mut screen = AddressDetailScreen::with_factories_and_tab(
        Chain::Ethereum,
        usdc(),
        feed,
        None,
        None,
        AddressTab::Contract,
    )
    .with_cursor_services(services);
    screen.set_overview_for_test(contract_overview());
    screen.set_contract_overview_for_test(ContractOverview {
        account: contract_overview(),
        proxy: None,
    });
    screen.set_contract_source_for_test(minimal_abi_source());
    screen.handle_key(key(KeyCode::Char(']')));
    screen.handle_key(key(KeyCode::Char(']')));
    assert_eq!(screen.active_contract_sub(), ContractSubTab::Abi);
    screen.handle_key(key(KeyCode::Char('y')));
    let copied = clipboard.last_copied().expect("clipboard");
    assert!(
        copied.contains("symbol"),
        "expected pretty-printed ABI, got {copied:?}"
    );
    assert!(
        copied.contains('\n'),
        "expected multi-line pretty JSON, got {copied:?}"
    );
}
