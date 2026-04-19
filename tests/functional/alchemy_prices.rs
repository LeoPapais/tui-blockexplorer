//! Wiremock tests for the [`AlchemyPrices`] REST adapter.
//!
//! Exercises both endpoints (`tokens/by-address` and
//! `tokens/historical`) and checks that an Alchemy 4xx response
//! maps onto `DomainError::Internal` instead of blowing up the
//! deserializer.
//!
//! See `plan/8-token-detail.md` section 12.4.

use assert_matches::assert_matches;
use blockexplorer_tui::{
    adapters::prices::{AlchemyPrices, PricesClient},
    application::ports::PricesPort,
    domain::{Address, Chain, PriceLookup, PriceWindow},
};
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use crate::support::fixture_loader::load_text;

fn adapter_for(url: &str) -> AlchemyPrices {
    let base = Url::parse(&format!("{url}/")).expect("base url parses");
    AlchemyPrices::new(PricesClient::new(base, reqwest::Client::new()))
}

fn usdc() -> Address {
    Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
}

#[tokio::test]
async fn spot_price_returns_some_when_provider_has_data() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/by-address"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("prices__by_address__usdc.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let lookup = adapter
        .get_single(usdc(), Chain::Ethereum)
        .await
        .expect("ok");

    assert_matches!(lookup, PriceLookup::Available(p) => {
        assert_eq!(p.currency, "usd");
        assert!((p.value - 1.0001).abs() < 1e-9);
        // 2024-04-05T12:00:00Z = 1712318400
        assert_eq!(p.as_of.seconds(), 1_712_318_400);
    });
}

#[tokio::test]
async fn spot_price_returns_unsupported_when_provider_has_no_prices() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/by-address"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("prices__by_address__unknown.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let lookup = adapter
        .get_single(
            Address::from_hex("0x0000000000000000000000000000000000000099").unwrap(),
            Chain::Ethereum,
        )
        .await
        .expect("ok");

    assert_matches!(
        lookup,
        PriceLookup::Unsupported { provider } if provider == "alchemy-prices"
    );
}

#[tokio::test]
async fn spot_price_returns_unsupported_when_provider_returns_404() {
    // plan/15-backlog.md §3.4: BRLA is indexed nowhere on Alchemy
    // Prices, so the REST endpoint answers 404. The adapter must map
    // that into a PriceLookup::Unsupported{ provider: "alchemy-prices" }
    // instead of crashing the deserializer.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/by-address"))
        .respond_with(ResponseTemplate::new(404).set_body_raw(
            load_text("prices__single__brla_not_indexed.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let lookup = adapter
        .get_single(
            Address::from_hex("0xe6a537a407488807f0bbeb0038b79004f19dddfb").unwrap(),
            Chain::Ethereum,
        )
        .await
        .expect("ok");

    assert_matches!(
        lookup,
        PriceLookup::Unsupported { provider } if provider == "alchemy-prices"
    );
}

#[tokio::test]
async fn historical_series_is_parsed_and_chronological() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/historical"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("prices__historical__usdc_1d.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let series = adapter
        .get_history(usdc(), Chain::Ethereum, PriceWindow::D1)
        .await
        .expect("ok");

    assert_eq!(series.window, PriceWindow::D1);
    assert_eq!(series.currency, "usd");
    assert_eq!(series.points.len(), 4);

    let ts: Vec<u64> = series.points.iter().map(|p| p.at.seconds()).collect();
    let mut sorted = ts.clone();
    sorted.sort();
    assert_eq!(ts, sorted, "points must be sorted ascending");
}

#[tokio::test]
async fn historical_request_sends_epoch_numbers_not_strings() {
    // Regression: the Alchemy Prices API rejects string-encoded
    // timestamps; it expects either an ISO 8601 string or a bare
    // JSON number. Previously we stringified `u64::to_string()`,
    // which produced a string that fails Alchemy's schema check
    // and left the Chart tab silently empty.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/historical"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("prices__historical__usdc_1d.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let _ = adapter
        .get_history(usdc(), Chain::Ethereum, PriceWindow::M1)
        .await
        .expect("ok");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "single historical POST");
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    let start = body.get("startTime").expect("startTime present");
    let end = body.get("endTime").expect("endTime present");
    assert!(
        start.is_number(),
        "startTime must be a JSON number, got {start:?}",
    );
    assert!(
        end.is_number(),
        "endTime must be a JSON number, got {end:?}",
    );
    assert_eq!(
        body.get("interval").and_then(|v| v.as_str()),
        Some("1d"),
        "interval must match PriceWindow::M1.alchemy_interval()",
    );
    assert_eq!(
        body.get("network").and_then(|v| v.as_str()),
        Some("eth-mainnet"),
    );
}

#[tokio::test]
async fn server_5xx_error_maps_into_provider_unavailable() {
    // plan/15-backlog.md §8.16 "Global error fixtures": every
    // adapter must map an HTTP 5xx into
    // `DomainError::ProviderUnavailable` so the degraded-state UX
    // and the per-client circuit breaker can treat it uniformly.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/by-address"))
        .respond_with(ResponseTemplate::new(500).set_body_raw(
            load_text("prices__error__5xx.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let err = adapter
        .get_single(usdc(), Chain::Ethereum)
        .await
        .expect_err("should propagate as error");

    use blockexplorer_tui::domain::DomainError;
    assert_matches!(err, DomainError::ProviderUnavailable);
}
