//! Functional tests for the `load_address_portfolio` use case.
//!
//! See `plan/6-address-detail.md` section 12.4.2.

use blockexplorer_tui::{
    application::use_cases::load_address_portfolio,
    domain::{Address, Chain, TokenHolding, TokenMetadata, Wei},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::StubPortfolioPort;

fn holding(symbol: &str, balance: u128) -> TokenHolding {
    TokenHolding {
        metadata: TokenMetadata {
            address: Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
            symbol: symbol.into(),
            name: format!("{symbol} Token"),
            decimals: 6,
        },
        balance: Wei::new(balance),
    }
}

#[tokio::test]
async fn returns_primed_holdings() {
    let port = StubPortfolioPort::new();
    let address = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    let holdings = vec![holding("USDC", 1_000_000), holding("USDT", 42)];
    port.set_holdings(address, holdings.clone());

    let got = load_address_portfolio::run(&port, address, Chain::Ethereum)
        .await
        .expect("ok");
    assert_eq!(got, holdings);
}

#[tokio::test]
async fn missing_address_returns_empty() {
    let port = StubPortfolioPort::new();
    let address = Address::from_hex("0x0000000000000000000000000000000000000099").unwrap();

    let got = load_address_portfolio::run(&port, address, Chain::Ethereum)
        .await
        .expect("ok");
    assert!(got.is_empty());
}
