//! Token sub-tab tests on the unified `AddressDetailScreen`.
//! Migrated from the deleted `tests/functional/token_detail_screen_keys.rs`.
//!
//! Covers the plan/8 §13.2 incomplete-token shortcut (now switches
//! to the Contract sub-tab in-place) and the plan/8 §13.1 live
//! streaming path (each `Available` sample appended to the active
//! window series).

use blockexplorer_tui::adapters::ui::{
    AddressDetailScreen, AddressTab, ContractSubTab, Screen, address_feed,
};
use blockexplorer_tui::domain::{
    Address, AddressKind, AddressOverview, Chain, PriceLookup, PricePoint, PriceSeries,
    PriceWindow, TokenMetadata, TokenOverview, TokenPrice, UnixTimestamp, Wei,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn addr() -> Address {
    Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
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

fn standard_overview() -> TokenOverview {
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

fn incomplete_overview() -> TokenOverview {
    TokenOverview {
        metadata: TokenMetadata {
            address: addr(),
            symbol: String::new(),
            name: String::new(),
            decimals: 0,
        },
        total_supply: 0,
        price: PriceLookup::Pending,
    }
}

#[test]
fn is_incomplete_badge_is_inactive_by_default() {
    let (feed, sender) = address_feed();
    let mut screen = AddressDetailScreen::loading(Chain::Ethereum, addr(), feed);
    screen.set_overview_for_test(contract_overview());
    sender
        .token_overview_tx
        .send(Some(standard_overview()))
        .unwrap();
    let _ = screen.tick();
    assert!(!screen.is_incomplete_badge_active());
}

#[test]
fn is_incomplete_badge_turns_on_when_metadata_is_degenerate() {
    let (feed, sender) = address_feed();
    let mut screen = AddressDetailScreen::loading(Chain::Ethereum, addr(), feed);
    screen.set_overview_for_test(contract_overview());
    sender
        .token_overview_tx
        .send(Some(incomplete_overview()))
        .unwrap();
    let _ = screen.tick();
    assert!(screen.is_incomplete_badge_active());
}

#[test]
fn c_key_switches_to_contract_subtab_when_badge_is_active() {
    let (feed, sender) = address_feed();
    let mut screen = AddressDetailScreen::with_factories_and_tab(
        Chain::Ethereum,
        addr(),
        feed,
        None,
        None,
        AddressTab::Token,
    );
    screen.set_overview_for_test(contract_overview());
    sender
        .token_overview_tx
        .send(Some(incomplete_overview()))
        .unwrap();
    let _ = screen.tick();

    screen.handle_key(key(KeyCode::Char('c')));
    assert_eq!(screen.active_tab(), AddressTab::Contract);
    assert_eq!(screen.active_contract_sub(), ContractSubTab::Overview);
}

#[test]
fn c_key_is_inert_when_badge_is_inactive() {
    let (feed, sender) = address_feed();
    let mut screen = AddressDetailScreen::with_factories_and_tab(
        Chain::Ethereum,
        addr(),
        feed,
        None,
        None,
        AddressTab::Token,
    );
    screen.set_overview_for_test(contract_overview());
    sender
        .token_overview_tx
        .send(Some(standard_overview()))
        .unwrap();
    let _ = screen.tick();

    // Token tab is the active tab; `c` is currently bound to
    // Plain( 'c' ) when the incomplete badge is active. When the
    // badge is inactive, `c` should be a no-op and stay on Token.
    screen.handle_key(key(KeyCode::Char('c')));
    assert_eq!(screen.active_tab(), AddressTab::Token);
}

fn sample_price(value: f64) -> TokenPrice {
    TokenPrice {
        currency: "usd".into(),
        value,
        as_of: UnixTimestamp::from_seconds(1_700_000_000),
    }
}

#[test]
fn streamed_available_samples_append_to_active_window_series() {
    let (feed, sender) = address_feed();
    let mut screen = AddressDetailScreen::loading(Chain::Ethereum, addr(), feed);
    screen.set_overview_for_test(contract_overview());
    sender
        .token_overview_tx
        .send(Some(standard_overview()))
        .unwrap();
    sender
        .token_series_tx
        .send(PriceSeries::empty(PriceWindow::D1))
        .unwrap();
    let _ = screen.tick();
    assert_eq!(screen.live_samples_count(), 0);

    sender
        .token_price_tx
        .send(PriceLookup::Available(sample_price(1.0001)))
        .unwrap();
    let _ = screen.tick();
    assert_eq!(screen.live_samples_count(), 1);
    let series = screen.token_series().expect("series present");
    assert_eq!(series.points.len(), 1);

    sender
        .token_price_tx
        .send(PriceLookup::Available(sample_price(1.25)))
        .unwrap();
    let _ = screen.tick();
    assert_eq!(screen.live_samples_count(), 2);
    let series = screen.token_series().expect("series present");
    assert_eq!(series.points.len(), 2);
    let price = screen.token_price().expect("available");
    assert!((price.value - 1.25).abs() < 1e-9);
}

#[test]
fn streamed_unsupported_refreshes_overview_without_touching_chart() {
    let (feed, sender) = address_feed();
    let mut screen = AddressDetailScreen::loading(Chain::Ethereum, addr(), feed);
    screen.set_overview_for_test(contract_overview());
    sender
        .token_overview_tx
        .send(Some(standard_overview()))
        .unwrap();
    sender
        .token_series_tx
        .send(PriceSeries::empty(PriceWindow::D1))
        .unwrap();
    let _ = screen.tick();

    sender
        .token_price_tx
        .send(PriceLookup::Unsupported {
            provider: "alchemy-prices",
        })
        .unwrap();
    let _ = screen.tick();

    assert_eq!(screen.live_samples_count(), 0);
    assert!(matches!(
        screen.token_price_lookup(),
        PriceLookup::Unsupported { provider } if *provider == "alchemy-prices"
    ));
    assert_eq!(screen.token_series().map(|s| s.points.len()), Some(0));
}

#[test]
fn live_tail_is_capped_at_rolling_cap() {
    let (feed, sender) = address_feed();
    let mut screen = AddressDetailScreen::loading(Chain::Ethereum, addr(), feed);
    screen.set_overview_for_test(contract_overview());
    sender
        .token_overview_tx
        .send(Some(standard_overview()))
        .unwrap();
    let mut seed = PriceSeries::empty(PriceWindow::D1);
    seed.points = (0..PriceSeries::ROLLING_CAP - 1)
        .map(|i| PricePoint {
            at: UnixTimestamp::from_seconds(i as u64),
            value: i as f64,
        })
        .collect();
    sender.token_series_tx.send(seed).unwrap();
    let _ = screen.tick();

    for _ in 0..5 {
        sender
            .token_price_tx
            .send(PriceLookup::Available(sample_price(2.0)))
            .unwrap();
    }
    let _ = screen.tick();

    let series = screen.token_series().expect("series present");
    assert_eq!(series.points.len(), PriceSeries::ROLLING_CAP);
}
