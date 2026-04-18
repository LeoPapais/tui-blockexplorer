//! Functional tests for `load_token_price`.
//!
//! See `plan/8-token-detail.md` section 4.4.

use assert_matches::assert_matches;
use blockexplorer_tui::{
    application::use_cases::load_token_price,
    domain::{Address, Chain, PriceLookup, TokenPrice, UnixTimestamp},
};

use crate::support::stubs::StubPricesPort;

fn usdc() -> Address {
    Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
}

#[tokio::test]
async fn happy_path_returns_spot_price() {
    let prices = StubPricesPort::new();
    prices.set_single(
        usdc(),
        TokenPrice {
            currency: "usd".into(),
            value: 1.0001,
            as_of: UnixTimestamp::from_seconds(1_700_000_000),
        },
    );

    let got = load_token_price::run(&prices, usdc(), Chain::Ethereum)
        .await
        .expect("ok");

    assert_matches!(got, PriceLookup::Available(TokenPrice { ref currency, value, .. }) if currency == "usd" && (value - 1.0001).abs() < 1e-9);
}

#[tokio::test]
async fn unknown_token_returns_unsupported() {
    let prices = StubPricesPort::new();
    let unknown = Address::from_hex("0x0000000000000000000000000000000000000099").unwrap();
    prices.set_unsupported(unknown, "alchemy-prices");

    let got = load_token_price::run(&prices, unknown, Chain::Ethereum)
        .await
        .expect("ok");

    assert_matches!(got, PriceLookup::Unsupported { provider } if provider == "alchemy-prices");
}

#[tokio::test]
async fn no_prime_defaults_to_unsupported() {
    // When the stub has no data for an address, it mirrors the real
    // Alchemy Prices adapter behaviour on a 404 / empty response:
    // the lookup is marked Unsupported with the alchemy-prices
    // provider. See plan/15-backlog.md §3.4.
    let prices = StubPricesPort::new();
    let unknown = Address::from_hex("0x000000000000000000000000000000000000dead").unwrap();

    let got = load_token_price::run(&prices, unknown, Chain::Ethereum)
        .await
        .expect("ok");

    assert_matches!(got, PriceLookup::Unsupported { provider } if provider == "alchemy-prices");
}
