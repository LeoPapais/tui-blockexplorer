//! Functional tests for `load_token_price`.
//!
//! See `plan/8-token-detail.md` section 4.4.

use blockexplorer_tui::{
    application::use_cases::load_token_price,
    domain::{Address, Chain, TokenPrice, UnixTimestamp},
};
use pretty_assertions::assert_eq;

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
        .expect("ok")
        .expect("some");

    assert_eq!(got.currency, "usd");
    assert!((got.value - 1.0001).abs() < 1e-9);
}

#[tokio::test]
async fn unknown_token_returns_none() {
    let prices = StubPricesPort::new();
    let unknown =
        Address::from_hex("0x0000000000000000000000000000000000000099").unwrap();

    let got = load_token_price::run(&prices, unknown, Chain::Ethereum)
        .await
        .expect("ok");

    assert!(got.is_none());
}
