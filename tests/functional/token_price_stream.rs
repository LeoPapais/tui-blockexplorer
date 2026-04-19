//! Functional tests for [`PollingTokenPriceStream`] and
//! [`StubTokenPriceStreamPort`].
//!
//! See `plan/8-token-detail.md` §13.1 and `plan/15-backlog.md` §8.9.

use std::time::Duration;

use assert_matches::assert_matches;
use blockexplorer_tui::{
    adapters::prices::PollingTokenPriceStream,
    application::ports::TokenPriceStreamPort,
    domain::{Address, Chain, DomainError, PriceLookup, TokenPrice, UnixTimestamp},
};

use crate::support::stubs::{StubPricesPort, StubTokenPriceStreamPort};

fn usdc() -> Address {
    Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
}

fn sample_price(value: f64) -> TokenPrice {
    TokenPrice {
        currency: "usd".into(),
        value,
        as_of: UnixTimestamp::from_seconds(1_700_000_000),
    }
}

#[tokio::test]
async fn polling_stream_emits_a_warm_up_sample_immediately() {
    let prices = StubPricesPort::new();
    let addr = usdc();
    prices.set_single(addr, sample_price(1.0001));

    let stream = PollingTokenPriceStream::new(prices, Duration::from_millis(50));
    let mut rx = stream.subscribe(addr, Chain::Ethereum).await.expect("sub");

    let first = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("warm-up arrived within 1s")
        .expect("sender still alive");
    assert_matches!(
        first,
        PriceLookup::Available(p) if (p.value - 1.0001).abs() < 1e-9
    );
}

#[tokio::test]
async fn polling_stream_delivers_two_samples_over_time() {
    let prices = StubPricesPort::new();
    let addr = usdc();
    prices.set_single(addr, sample_price(1.0));

    let stream = PollingTokenPriceStream::new(prices, Duration::from_millis(30));
    let mut rx = stream.subscribe(addr, Chain::Ethereum).await.expect("sub");

    let _ = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("first sample");
    let second = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("second sample within 1s")
        .expect("sender still alive");
    assert_matches!(second, PriceLookup::Available(_));
}

#[tokio::test]
async fn polling_stream_surfaces_unsupported_when_underlying_port_is_unindexed() {
    let prices = StubPricesPort::new();
    let addr = Address::from_hex("0xe6a537a407488807f0bbeb0038b79004f19dddfb").unwrap();
    prices.set_unsupported(addr, "alchemy-prices");

    let stream = PollingTokenPriceStream::new(prices, Duration::from_millis(50));
    let mut rx = stream.subscribe(addr, Chain::Ethereum).await.expect("sub");

    let first = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("first sample")
        .expect("sender alive");
    assert_matches!(
        first,
        PriceLookup::Unsupported { provider } if provider == "alchemy-prices"
    );
}

#[tokio::test]
async fn stub_broadcasts_pushed_prices_to_every_subscriber() {
    let stream = StubTokenPriceStreamPort::new();
    let addr = usdc();
    let mut rx1 = stream.subscribe(addr, Chain::Ethereum).await.expect("sub1");
    let mut rx2 = stream.subscribe(addr, Chain::Ethereum).await.expect("sub2");

    stream.push(addr, sample_price(1.25));

    let a = rx1.recv().await.expect("rx1 got sample");
    let b = rx2.recv().await.expect("rx2 got sample");
    assert_matches!(a, PriceLookup::Available(p) if (p.value - 1.25).abs() < 1e-9);
    assert_matches!(b, PriceLookup::Available(p) if (p.value - 1.25).abs() < 1e-9);
}

#[tokio::test]
async fn stub_rejects_subscribe_when_broken() {
    let stream = StubTokenPriceStreamPort::new();
    stream.set_broken(true);

    let err = stream
        .subscribe(usdc(), Chain::Ethereum)
        .await
        .expect_err("subscribe must fail when broken");
    assert_matches!(err, DomainError::ProviderUnavailable);
}
