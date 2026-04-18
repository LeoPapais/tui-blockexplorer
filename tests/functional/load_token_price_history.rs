//! Functional tests for `load_token_price_history`.
//!
//! See `plan/8-token-detail.md` section 4.3.

use blockexplorer_tui::{
    application::use_cases::load_token_price_history,
    domain::{Address, Chain, PricePoint, PriceSeries, PriceWindow, UnixTimestamp},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::StubPricesPort;

fn usdc() -> Address {
    Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
}

fn series(window: PriceWindow, n: u64) -> PriceSeries {
    let points = (0..n)
        .map(|i| PricePoint {
            at: UnixTimestamp::from_seconds(1_700_000_000 + i * 3600),
            value: 1.0 + (i as f64) * 0.01,
        })
        .collect();
    PriceSeries {
        window,
        currency: "usd".into(),
        points,
    }
}

#[tokio::test]
async fn happy_path_returns_requested_window_series() {
    let prices = StubPricesPort::new();
    for window in [PriceWindow::D1, PriceWindow::M1, PriceWindow::Y1] {
        prices.set_history(usdc(), series(window, 4));
    }

    let got = load_token_price_history::run(&prices, usdc(), Chain::Ethereum, PriceWindow::M1)
        .await
        .expect("ok");

    assert_eq!(got.window, PriceWindow::M1);
    assert_eq!(got.points.len(), 4);
    assert_eq!(got.currency, "usd");
}

#[tokio::test]
async fn missing_history_returns_empty_series_not_error() {
    let prices = StubPricesPort::new();

    let got = load_token_price_history::run(&prices, usdc(), Chain::Ethereum, PriceWindow::Y1)
        .await
        .expect("empty series must not be an error");

    assert_eq!(got.window, PriceWindow::Y1);
    assert!(got.points.is_empty());
}
