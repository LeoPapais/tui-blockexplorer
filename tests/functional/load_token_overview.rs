//! Functional tests for `load_token_overview`.
//!
//! See `plan/8-token-detail.md` section 12.1.

use assert_matches::assert_matches;
use blockexplorer_tui::{
    application::use_cases::load_token_overview,
    domain::{Address, Chain, DomainError, PriceLookup, TokenMetadata, TokenOverview},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::{StubPricesPort, StubTokenReaderPort};

fn sample() -> TokenOverview {
    TokenOverview {
        metadata: TokenMetadata {
            address: Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
            symbol: "USDC".to_string(),
            name: "USD Coin".to_string(),
            decimals: 6,
        },
        total_supply: 35_200_000_000_000_000_u128,
        price: PriceLookup::Pending,
    }
}

#[tokio::test]
async fn happy_path_returns_overview() {
    let reader = StubTokenReaderPort::new();
    let prices = StubPricesPort::new();
    let ov = sample();
    reader.insert(ov.clone());
    prices.set_single(
        ov.metadata.address,
        blockexplorer_tui::domain::TokenPrice {
            currency: "usd".into(),
            value: 1.0001,
            as_of: blockexplorer_tui::domain::UnixTimestamp::from_seconds(1),
        },
    );

    let got =
        load_token_overview::run(&reader, &prices, ov.metadata.address, Chain::Ethereum)
            .await
            .expect("ok");

    assert_eq!(got.metadata, ov.metadata);
    assert_eq!(got.total_supply, 35_200_000_000_000_000);
    assert_matches!(
        got.price,
        PriceLookup::Available(blockexplorer_tui::domain::TokenPrice { ref currency, value, .. })
            if currency == "usd" && (value - 1.0001).abs() < 1e-9,
    );
}

#[tokio::test]
async fn missing_token_returns_not_found() {
    let reader = StubTokenReaderPort::new();
    let prices = StubPricesPort::new();
    let addr = Address::from_hex("0x0000000000000000000000000000000000000001").unwrap();

    let err = load_token_overview::run(&reader, &prices, addr, Chain::Ethereum)
        .await
        .expect_err("missing token must error");

    assert!(matches!(err, DomainError::NotFound));
}

#[tokio::test]
async fn returns_unsupported_when_prices_api_returns_404() {
    // plan/15-backlog.md §3.4: BRLA is an ERC-20 the Alchemy Prices
    // API does not index. The use case must surface that as an
    // Unsupported lookup instead of leaving the price blank.
    let reader = StubTokenReaderPort::new();
    let prices = StubPricesPort::new();
    let brla = Address::from_hex("0xe6a537a407488807f0bbeb0038b79004f19dddfb").unwrap();
    reader.insert(TokenOverview {
        metadata: TokenMetadata {
            address: brla,
            symbol: "BRLA".into(),
            name: "BRLA Token".into(),
            decimals: 18,
            },
        total_supply: 1_000_000_000_000_000_000_000_u128,
        price: PriceLookup::Pending,
    });
    prices.set_unsupported(brla, "alchemy-prices");

    let got = load_token_overview::run(&reader, &prices, brla, Chain::Ethereum)
        .await
        .expect("ok");

    assert_matches!(
        got.price,
        PriceLookup::Unsupported { provider } if provider == "alchemy-prices",
    );
}
