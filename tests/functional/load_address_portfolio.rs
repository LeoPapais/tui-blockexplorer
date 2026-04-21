//! Functional tests for the `load_address_portfolio` use case.
//!
//! See `plan/6-address-detail.md` section 12.4.2 and
//! `plan/15-backlog.md` §3.4 for the per-holding PriceLookup wiring.

use assert_matches::assert_matches;
use blockexplorer_tui::{
    application::use_cases::load_address_portfolio,
    domain::{
        Address, Chain, PriceLookup, TokenHolding, TokenMetadata, TokenPrice, UnixTimestamp, Wei,
    },
};
use pretty_assertions::assert_eq;

use crate::support::stubs::{StubPortfolioPort, StubPricesPort};

fn addr(hex: &str) -> Address {
    Address::from_hex(hex).unwrap()
}

fn holding(contract: Address, symbol: &str, balance: u128) -> TokenHolding {
    TokenHolding {
        metadata: TokenMetadata {
            address: contract,
            symbol: symbol.into(),
            name: format!("{symbol} Token"),
            decimals: 6,
        },
        balance: Wei::new(balance),
        price: PriceLookup::Pending,
    }
}

#[tokio::test]
async fn returns_primed_holdings() {
    let port = StubPortfolioPort::new();
    let prices = StubPricesPort::new();
    let owner = addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
    let usdc = addr("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let usdt = addr("0xdac17f958d2ee523a2206206994597c13d831ec7");
    port.set_holdings(
        owner,
        vec![holding(usdc, "USDC", 1_000_000), holding(usdt, "USDT", 42)],
    );
    prices.set_single(
        usdc,
        TokenPrice {
            currency: "usd".into(),
            value: 1.0,
            as_of: UnixTimestamp::from_seconds(1),
        },
    );
    prices.set_single(
        usdt,
        TokenPrice {
            currency: "usd".into(),
            value: 0.999,
            as_of: UnixTimestamp::from_seconds(1),
        },
    );

    let got = load_address_portfolio::run(&port, &prices, owner, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got.len(), 2);
    assert_eq!(got[0].metadata.symbol, "USDC");
    assert_matches!(got[0].price, PriceLookup::Available(ref p) if (p.value - 1.0).abs() < 1e-9);
    assert_matches!(got[1].price, PriceLookup::Available(ref p) if (p.value - 0.999).abs() < 1e-9);
}

#[tokio::test]
async fn missing_address_returns_empty() {
    let port = StubPortfolioPort::new();
    let prices = StubPricesPort::new();
    let address = addr("0x0000000000000000000000000000000000000099");

    let got = load_address_portfolio::run(&port, &prices, address, Chain::Ethereum)
        .await
        .expect("ok");
    assert!(got.is_empty());
}

#[tokio::test]
async fn unsupported_price_still_renders_holding() {
    // plan/15-backlog.md §3.4: one token in the portfolio has a real
    // spot price; another (BRLA) is not indexed by the Prices API.
    // The portfolio must keep both rows and tag BRLA's price as
    // Unsupported so the UI can render the badge.
    let port = StubPortfolioPort::new();
    let prices = StubPricesPort::new();
    let owner = addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
    let usdc = addr("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let brla = addr("0xe6a537a407488807f0bbeb0038b79004f19dddfb");
    port.set_holdings(
        owner,
        vec![
            holding(usdc, "USDC", 1_000_000),
            holding(brla, "BRLA", 42_000_000),
        ],
    );
    prices.set_single(
        usdc,
        TokenPrice {
            currency: "usd".into(),
            value: 1.0,
            as_of: UnixTimestamp::from_seconds(1),
        },
    );
    prices.set_unsupported(brla, "alchemy-prices");

    let got = load_address_portfolio::run(&port, &prices, owner, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got.len(), 2);
    assert_matches!(got[0].price, PriceLookup::Available(_));
    assert_matches!(
        got[1].price,
        PriceLookup::Unsupported { provider } if provider == "alchemy-prices"
    );
}

/// plan/18 Slice C: portfolio port may return more than twenty rows; the
/// use case must forward every row to the UI (adapter caps metadata fan-out).
#[tokio::test]
async fn forwards_all_rows_when_stub_returns_more_than_twenty_holdings() {
    let port = StubPortfolioPort::new();
    let prices = StubPricesPort::new();
    let owner = addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
    let mut rows = Vec::new();
    for i in 0..25u8 {
        let mut bytes = [0u8; 20];
        bytes[0] = 0xde;
        bytes[1] = 0xad;
        bytes[19] = i;
        let contract = Address::from_bytes(bytes);
        rows.push(TokenHolding {
            metadata: TokenMetadata {
                address: contract,
                symbol: format!("T{i}"),
                name: format!("Token {i}"),
                decimals: 6,
            },
            balance: Wei::new(100 + u128::from(i)),
            price: PriceLookup::Pending,
        });
        prices.set_single(
            contract,
            TokenPrice {
                currency: "usd".into(),
                value: 1.0,
                as_of: UnixTimestamp::from_seconds(1),
            },
        );
    }
    port.set_holdings(owner, rows);

    let got = load_address_portfolio::run(&port, &prices, owner, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got.len(), 25);
}
