//! Functional tests for `load_token_overview`.
//!
//! See `plan/8-token-detail.md` section 12.1.

use blockexplorer_tui::{
    application::use_cases::load_token_overview,
    domain::{Address, Chain, DomainError, TokenMetadata, TokenOverview},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::StubTokenReaderPort;

fn sample() -> TokenOverview {
    TokenOverview {
        metadata: TokenMetadata {
            address: Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
            symbol: "USDC".to_string(),
            name: "USD Coin".to_string(),
            decimals: 6,
        },
        total_supply: 35_200_000_000_000_000_u128,
        price: None,
    }
}

#[tokio::test]
async fn happy_path_returns_overview() {
    let reader = StubTokenReaderPort::new();
    let ov = sample();
    reader.insert(ov.clone());

    let got = load_token_overview::run(&reader, ov.metadata.address, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got, ov);
    assert_eq!(got.metadata.symbol, "USDC");
    assert_eq!(got.total_supply, 35_200_000_000_000_000);
}

#[tokio::test]
async fn missing_token_returns_not_found() {
    let reader = StubTokenReaderPort::new();
    let addr = Address::from_hex("0x0000000000000000000000000000000000000001").unwrap();

    let err = load_token_overview::run(&reader, addr, Chain::Ethereum)
        .await
        .expect_err("missing token must error");

    assert!(matches!(err, DomainError::NotFound));
}
