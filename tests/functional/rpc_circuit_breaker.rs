//! Functional tests for the per-`RpcClient` circuit breaker.
//!
//! See `plan/13-alchemy-adapter.md` §8.3 and
//! `plan/15-backlog.md` §8.14 item 3.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use blockexplorer_tui::{
    adapters::rpc::{
        CircuitBreaker, CircuitBreakerConfig, RetryPolicy, RpcClient, RpcError,
    },
    application::ports::Clock,
    domain::DomainError,
};
use pretty_assertions::assert_eq;
use serde_json::json;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::method,
};

use crate::support::fixture_loader::load_text;
use crate::support::stubs::{FrozenClock, SeededRng};

fn short_retry_policy() -> RetryPolicy {
    let rng: Arc<dyn blockexplorer_tui::application::ports::Rng> = Arc::new(SeededRng::new(17));
    RetryPolicy::new(
        1, // no retries: we want each logical call to count as one failure
        Duration::from_millis(0),
        Duration::from_millis(0),
        rng,
    )
}

fn build_client(url: &str, breaker: Arc<CircuitBreaker>) -> RpcClient {
    RpcClient::new(Url::parse(url).unwrap(), reqwest::Client::new())
        .with_retry_policy(short_retry_policy())
        .with_circuit_breaker(breaker)
}

#[tokio::test]
async fn fast_fails_with_circuit_open_after_threshold_consecutive_failures() {
    let server = MockServer::start().await;
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_inner = hits.clone();
    Mock::given(method("POST"))
        .respond_with(move |_req: &wiremock::Request| {
            hits_inner.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(500)
        })
        .mount(&server)
        .await;

    let clock: Arc<dyn Clock> = Arc::new(FrozenClock::default());
    let breaker = Arc::new(CircuitBreaker::new(
        CircuitBreakerConfig {
            failure_threshold: 3,
            cool_down: Duration::from_secs(10),
        },
        clock,
    ));
    let client = build_client(&server.uri(), breaker.clone());

    // Trip the breaker.
    for _ in 0..3 {
        let err = client
            .call::<_, String>("eth_blockNumber", json!([]))
            .await
            .expect_err("500 response is a failure");
        assert!(matches!(err, RpcError::HttpServerError { status: 500 }));
    }
    assert_eq!(hits.load(Ordering::SeqCst), 3);
    assert!(breaker.is_open(), "breaker must be open after threshold");

    // Next call short-circuits; the mock server must not see any new
    // hit.
    let err = client
        .call::<_, String>("eth_blockNumber", json!([]))
        .await
        .expect_err("breaker open");
    assert!(matches!(err, RpcError::CircuitOpen));
    assert_eq!(
        hits.load(Ordering::SeqCst),
        3,
        "fast-fail must not reach the mock server",
    );
    assert!(matches!(
        err.into_domain(),
        DomainError::ProviderUnavailable,
    ));
}

#[tokio::test]
async fn reopens_and_closes_after_cool_down_elapses() {
    let server = MockServer::start().await;

    // Two failing responses, then a happy one.
    let fail_count = Arc::new(AtomicUsize::new(0));
    let fail_inner = fail_count.clone();
    Mock::given(method("POST"))
        .respond_with(move |_req: &wiremock::Request| {
            let n = fail_inner.fetch_add(1, Ordering::SeqCst);
            if n < 3 {
                ResponseTemplate::new(500)
            } else {
                ResponseTemplate::new(200).set_body_raw(
                    load_text("rpc__eth_blockNumber__ethereum.json"),
                    "application/json",
                )
            }
        })
        .mount(&server)
        .await;

    let clock = FrozenClock::default();
    let clock_arc: Arc<dyn Clock> = Arc::new(clock.clone());
    let breaker = Arc::new(CircuitBreaker::new(
        CircuitBreakerConfig {
            failure_threshold: 3,
            cool_down: Duration::from_millis(500),
        },
        clock_arc,
    ));
    let client = build_client(&server.uri(), breaker.clone());

    for _ in 0..3 {
        let _ = client
            .call::<_, String>("eth_blockNumber", json!([]))
            .await;
    }
    assert!(breaker.is_open());

    // Short-circuit while still open.
    assert!(matches!(
        client
            .call::<_, String>("eth_blockNumber", json!([]))
            .await,
        Err(RpcError::CircuitOpen),
    ));

    // Advance past the cool-down → half-open.
    clock.advance(Duration::from_millis(500));
    let value: String = client
        .call("eth_blockNumber", json!([]))
        .await
        .expect("probe call succeeds and closes the breaker");
    assert_eq!(value, "0x10");

    // Subsequent calls are served as normal; no short-circuit.
    let _value: String = client
        .call("eth_blockNumber", json!([]))
        .await
        .expect("breaker stays closed after successful probe");
    assert!(!breaker.is_open());
}

#[tokio::test]
async fn non_retryable_errors_do_not_trip_the_breaker() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__error__invalid_params.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let clock: Arc<dyn Clock> = Arc::new(FrozenClock::default());
    let breaker = Arc::new(CircuitBreaker::new(
        CircuitBreakerConfig {
            failure_threshold: 2,
            cool_down: Duration::from_secs(10),
        },
        clock,
    ));
    let client = build_client(&server.uri(), breaker.clone());

    for _ in 0..5 {
        let err = client
            .call::<_, String>("eth_foo", json!([]))
            .await
            .expect_err("invalid params");
        assert!(matches!(err, RpcError::Rpc { code: -32602, .. }));
    }
    assert!(
        !breaker.is_open(),
        "-32602 is non-retryable and must not trip the breaker",
    );
}
