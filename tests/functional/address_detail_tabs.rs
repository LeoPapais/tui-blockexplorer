//! Tab-bar shape and sub-tab cycling tests for the unified
//! `AddressDetailScreen`. See `plan/16-unified-address-detail.md`
//! §2 (tab tree) and §3 (keybindings).

use blockexplorer_tui::adapters::ui::{
    AddressDetailScreen, AddressTab, ContractSubTab, Screen, TokenSubTab, address_feed,
};
use blockexplorer_tui::domain::{
    Address, AddressKind, AddressOverview, Chain, PriceLookup, TokenMetadata, TokenOverview, Wei,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn addr() -> Address {
    Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
}

fn eoa_overview() -> AddressOverview {
    AddressOverview {
        chain: Chain::Ethereum,
        address: addr(),
        balance: Wei::new(0),
        nonce: 0,
        kind: AddressKind::Eoa { delegated_to: None },
        delegated_to: None,
        ens_name: None,
    }
}

fn contract_overview() -> AddressOverview {
    AddressOverview {
        chain: Chain::Ethereum,
        address: addr(),
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
            address: addr(),
            symbol: "USDC".into(),
            name: "USD Coin".into(),
            decimals: 6,
        },
        total_supply: 35_000_000_000_000,
        price: PriceLookup::Pending,
    }
}

fn build(ov: AddressOverview, token: Option<TokenOverview>) -> AddressDetailScreen {
    let (feed, _sender) = address_feed();
    let mut screen = AddressDetailScreen::loading(Chain::Ethereum, addr(), feed);
    screen.set_overview_for_test(ov);
    screen.set_token_overview_for_test(token);
    screen
}

#[test]
fn eoa_shows_four_main_tabs_and_no_subtabs() {
    let screen = build(eoa_overview(), None);
    let tabs = screen.tabs();
    assert_eq!(
        tabs,
        vec![
            AddressTab::Overview,
            AddressTab::Transactions,
            AddressTab::Transfers,
            AddressTab::Tokens
        ],
    );
}

#[test]
fn plain_contract_shows_five_main_tabs_with_contract_subtabs() {
    let mut screen = build(contract_overview(), None);
    // Simulate a "NotToken" probe result by flipping to None.
    // (Using `set_token_overview_for_test(None)` lands on NotToken.)
    let tabs = screen.tabs();
    assert_eq!(
        tabs,
        vec![
            AddressTab::Overview,
            AddressTab::Transactions,
            AddressTab::Transfers,
            AddressTab::Tokens,
            AddressTab::Contract,
        ],
    );
    // Default contract sub-tab is Overview; cycling visits every
    // sub-tab once.
    screen.handle_key(key(KeyCode::Tab));
    screen.handle_key(key(KeyCode::Tab));
    screen.handle_key(key(KeyCode::Tab));
    screen.handle_key(key(KeyCode::Tab)); // -> Contract main tab
    assert_eq!(screen.active_tab(), AddressTab::Contract);
    assert_eq!(screen.active_contract_sub(), ContractSubTab::Overview);
}

#[test]
fn erc20_shows_six_main_tabs_with_token_and_contract_subtabs() {
    let screen = build(contract_overview(), Some(token_overview()));
    let tabs = screen.tabs();
    assert_eq!(
        tabs,
        vec![
            AddressTab::Overview,
            AddressTab::Transactions,
            AddressTab::Transfers,
            AddressTab::Tokens,
            AddressTab::Token,
            AddressTab::Contract,
        ],
    );
}

#[test]
fn bracket_keys_rotate_contract_subtabs() {
    let mut screen = build(contract_overview(), None);
    // Navigate to the Contract main tab.
    for _ in 0..4 {
        screen.handle_key(key(KeyCode::Tab));
    }
    assert_eq!(screen.active_tab(), AddressTab::Contract);
    assert_eq!(screen.active_contract_sub(), ContractSubTab::Overview);

    // Cycle through every sub-tab with `]`.
    let expected = [
        ContractSubTab::Source,
        ContractSubTab::Abi,
        ContractSubTab::Read,
        ContractSubTab::Events,
        ContractSubTab::Storage,
        ContractSubTab::Overview,
    ];
    for &sub in &expected {
        screen.handle_key(key(KeyCode::Char(']')));
        assert_eq!(screen.active_contract_sub(), sub);
    }

    // `[` goes back.
    screen.handle_key(key(KeyCode::Char('[')));
    assert_eq!(screen.active_contract_sub(), ContractSubTab::Storage);
}

#[test]
fn bracket_keys_rotate_token_subtabs() {
    let mut screen = build(contract_overview(), Some(token_overview()));
    // Token main tab: Overview, Transactions, Transfers, Tokens, Token.
    for _ in 0..4 {
        screen.handle_key(key(KeyCode::Tab));
    }
    assert_eq!(screen.active_tab(), AddressTab::Token);
    assert_eq!(screen.active_token_sub(), TokenSubTab::Overview);

    let expected = [
        TokenSubTab::Transfers,
        TokenSubTab::Chart,
        TokenSubTab::Overview,
    ];
    for &sub in &expected {
        screen.handle_key(key(KeyCode::Char(']')));
        assert_eq!(screen.active_token_sub(), sub);
    }
}

#[test]
fn switching_to_contract_tab_defaults_to_overview_subtab() {
    let mut screen = build(contract_overview(), None);
    for _ in 0..4 {
        screen.handle_key(key(KeyCode::Tab));
    }
    assert_eq!(screen.active_tab(), AddressTab::Contract);
    assert_eq!(screen.active_contract_sub(), ContractSubTab::Overview);
}
