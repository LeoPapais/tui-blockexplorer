//! Functional tests for `RpcClient` retries under the
//! exponential-backoff policy. See `plan/13-alchemy-adapter.md` §8.2
//! and `plan/15-backlog.md` §8.14 item 2.
//!
//! Sequential responses are expressed via wiremock's
//! `Mock::up_to_n_times` stacking so the first call sees a 429, the
//! second sees a 5xx and the third sees a success.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use blockexplorer_tui::{
    adapters::rpc::{RetryPolicy, RpcClient},
    domain::DomainError,
};
use pretty_assertions::assert_eq;
use serde_json::json;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

use crate::support::fixture_loader::load_text;
use crate::support::stubs::SeededRng;

fn policy_for_tests() -> RetryPolicy {
    // Very short delays so tests complete in ~ms but the ordering of
    // retry vs non-retry is still exercised.
    let rng: Arc<dyn blockexplorer_tui::application::ports::Rng> = Arc::new(SeededRng::new(7));
    RetryPolicy::new(
        3,
        Duration::from_millis(1),
        Duration::from_millis(2),
        rng,
    )
}

fn client_for(url: &str) -> RpcClient {
    RpcClient::new(Url::parse(url).unwrap(), reqwest::Client::new()).with_retry_policy(policy_for_tests())
}

#[tokio::test]
async fn recovers_after_429_then_500_then_200() {
    let server = MockServer::start().await;

    // First attempt: HTTP 429
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method": "eth_blockNumber"})))
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Second attempt: HTTP 500
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method": "eth_blockNumber"})))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Third attempt: happy response
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method": "eth_blockNumber"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_blockNumber__ethereum.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let client = client_for(&server.uri());
    let value: String = client
        .call("eth_blockNumber", json!([]))
        .await
        .expect("third attempt succeeds");

    assert_eq!(value, "0x10");
}

#[tokio::test]
async fn exhausts_attempts_after_three_rate_limit_responses() {
    let server = MockServer::start().await;
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_inner = hits.clone();

    Mock::given(method("POST"))
        .respond_with(move |_req: &wiremock::Request| {
            hits_inner.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(200).set_body_raw(
                load_text("rpc__error__rate_limit.json"),
                "application/json",
            )
        })
        .mount(&server)
        .await;

    let client = client_for(&server.uri());
    let err = client
        .call::<_, String>("eth_blockNumber", json!([]))
        .await
        .expect_err("three -32005 responses exhaust retries");

    assert!(matches!(err, blockexplorer_tui::adapters::rpc::RpcError::Rpc { code: -32005, .. }));
    assert_eq!(
        hits.load(Ordering::SeqCst),
        3,
        "the policy must cap at 3 total attempts",
    );
}

#[tokio::test]
async fn never_retries_when_rpc_error_is_not_retryable() {
    let server = MockServer::start().await;
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_inner = hits.clone();

    Mock::given(method("POST"))
        .respond_with(move |_req: &wiremock::Request| {
            hits_inner.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(200).set_body_raw(
                load_text("rpc__error__invalid_params.json"),
                "application/json",
            )
        })
        .mount(&server)
        .await;

    let client = client_for(&server.uri());
    let err = client
        .call::<_, String>("eth_blockNumber", json!([]))
        .await
        .expect_err("invalid params must short-circuit");

    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "non-retryable errors must not consume retry budget",
    );
    // Raw RpcError carries -32602 / "invalid params"; the adapter
    // boundary later translates it into DomainError::InvalidInput
    // (pinned by tests/functional/rpc_error_mapping.rs).
    assert!(matches!(
        err,
        blockexplorer_tui::adapters::rpc::RpcError::Rpc { code: -32602, .. }
    ));
}

#[tokio::test]
async fn adapter_boundary_surfaces_provider_unavailable_when_retries_are_exhausted() {
    // Belt-and-suspenders scenario: three consecutive 5xx responses
    // go through `call_once`, exhaust the three attempts of the
    // retry policy and finally surface as `DomainError::ProviderUnavailable`
    // through `RpcError::into_domain`.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let client = client_for(&server.uri());
    let err = client
        .call::<_, String>("eth_blockNumber", json!([]))
        .await
        .expect_err("three 503 responses must exhaust retries");
    let domain: DomainError = err.into_domain();
    assert!(matches!(domain, DomainError::ProviderUnavailable));
}
