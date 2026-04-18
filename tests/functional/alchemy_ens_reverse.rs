//! Wiremock tests for the Alchemy-backed reverse ENS resolution.
//!
//! Drives the three-call flow spelled out in
//! `.cursor/rules/external-apis.mdc`:
//!
//! 1. `resolver(node)` on the ENS Registry for
//!    `<lower-hex>.addr.reverse`.
//! 2. `name(node)` on the returned resolver.
//! 3. Forward confirmation via `resolver(namehash)` + `addr(node)`.
//!
//! If the forward-confirm disagrees with the starting address, the
//! adapter reports no match (prevents a squatter from injecting a
//! misleading reverse record). See plan/6 §11 "Shipped".

use blockexplorer_tui::{
    adapters::rpc::{AlchemyEnsResolver, RpcClient},
    application::ports::EnsResolverPort,
    domain::{Address, Chain},
};
use reqwest::Client;
use serde_json::{Value, json};
use url::Url;
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate, matchers::method};

use crate::support::fixture_loader::load_text;

/// Registry used by every real ENS deployment on EVM mainnets.
const REGISTRY: &str = "0x00000000000c2e074ec69a0dfb2997ba6c7d2e1e";

/// Resolver contract returned by `rpc__eth_call__ens_resolver.json`.
const RESOLVER: &str = "0x4976fb03c32e5b8cfe2b6ccb31c09ba78ebaba41";

fn rpc(url: &str) -> RpcClient {
    RpcClient::new(Url::parse(url).unwrap(), Client::new())
}

/// Routes each `eth_call` to a fixture based on the target address
/// and the calldata selector. Keeps the test free of wiremock
/// chaining for the three-hop dance.
struct EnsReverseResponder {
    /// Fixture for the very last forward `addr(node)` lookup; used
    /// to drive the confirmation branch (match vs mismatch).
    forward_addr_fixture: &'static str,
}

impl Respond for EnsReverseResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        let method_name = body
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if method_name != "eth_call" {
            return ResponseTemplate::new(500);
        }
        let params = body
            .get("params")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let call = params.first().cloned().unwrap_or(Value::Null);
        let to = call
            .get("to")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let data = call
            .get("data")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let selector = data.get(..10).unwrap_or_default();

        // Registry resolver(bytes32) — 0x0178b8bf.
        if to == REGISTRY && selector == "0x0178b8bf" {
            return ResponseTemplate::new(200).set_body_raw(
                load_text("rpc__eth_call__ens_resolver.json"),
                "application/json",
            );
        }

        if to == RESOLVER {
            // Resolver name(bytes32) — 0x691f3431.
            if selector == "0x691f3431" {
                return ResponseTemplate::new(200).set_body_raw(
                    load_text("rpc__eth_call__ens_name_vitalik.json"),
                    "application/json",
                );
            }
            // Resolver addr(bytes32) — 0x3b3b57de — the forward
            // confirmation step. The fixture varies per test.
            if selector == "0x3b3b57de" {
                return ResponseTemplate::new(200)
                    .set_body_raw(load_text(self.forward_addr_fixture), "application/json");
            }
        }
        ResponseTemplate::new(500)
    }
}

#[tokio::test]
async fn reverse_returns_name_when_forward_confirms() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(EnsReverseResponder {
            forward_addr_fixture: "rpc__eth_call__ens_addr.json",
        })
        .mount(&server)
        .await;

    let adapter = AlchemyEnsResolver::new(rpc(&server.uri()));
    let vitalik = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

    let got = adapter
        .reverse(vitalik, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("matches");

    assert_eq!(got, "vitalik.eth");
}

#[tokio::test]
async fn reverse_returns_none_when_forward_disagrees() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(EnsReverseResponder {
            // Different address — forward confirmation must fail.
            forward_addr_fixture: "rpc__eth_call__ens_addr_mismatch.json",
        })
        .mount(&server)
        .await;

    let adapter = AlchemyEnsResolver::new(rpc(&server.uri()));
    let vitalik = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

    let got = adapter.reverse(vitalik, Chain::Ethereum).await.expect("ok");

    assert!(got.is_none(), "forward mismatch must yield None");
}

#[tokio::test]
async fn reverse_returns_none_when_resolver_is_zero() {
    // Registry returns the zero address for `resolver(node)` on the
    // reverse namehash — wallet has no reverse record at all.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(|req: &Request| -> ResponseTemplate {
            let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let call = body
                .get("params")
                .and_then(Value::as_array)
                .and_then(|p| p.first())
                .cloned()
                .unwrap_or(Value::Null);
            let to = call
                .get("to")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            if to == REGISTRY {
                ResponseTemplate::new(200).set_body_raw(
                    load_text("rpc__eth_call__ens_resolver_none.json"),
                    "application/json",
                )
            } else {
                ResponseTemplate::new(500)
                    .set_body_string(json!({"error":"unexpected"}).to_string())
            }
        })
        .mount(&server)
        .await;

    let adapter = AlchemyEnsResolver::new(rpc(&server.uri()));
    let naked = Address::from_hex("0x0000000000000000000000000000000000000099").unwrap();
    let got = adapter.reverse(naked, Chain::Ethereum).await.expect("ok");
    assert!(got.is_none());
}

#[tokio::test]
async fn reverse_returns_none_when_name_is_empty_string() {
    // Resolver exists but returns an empty ABI-encoded string for
    // `name(node)`. Mirrors the "stale resolver" scenario the
    // official ENS docs warn about.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(|req: &Request| -> ResponseTemplate {
            let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let call = body
                .get("params")
                .and_then(Value::as_array)
                .and_then(|p| p.first())
                .cloned()
                .unwrap_or(Value::Null);
            let to = call
                .get("to")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            let data = call
                .get("data")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            let selector = data.get(..10).unwrap_or_default();
            if to == REGISTRY {
                return ResponseTemplate::new(200).set_body_raw(
                    load_text("rpc__eth_call__ens_resolver.json"),
                    "application/json",
                );
            }
            if to == RESOLVER && selector == "0x691f3431" {
                return ResponseTemplate::new(200).set_body_raw(
                    load_text("rpc__eth_call__ens_name_empty.json"),
                    "application/json",
                );
            }
            ResponseTemplate::new(500)
        })
        .mount(&server)
        .await;

    let adapter = AlchemyEnsResolver::new(rpc(&server.uri()));
    let vitalik = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    let got = adapter.reverse(vitalik, Chain::Ethereum).await.expect("ok");
    assert!(got.is_none());
}

#[tokio::test]
async fn reverse_maps_rate_limit_to_domain_rate_limit() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_raw(load_text("rpc__error__rate_limit.json"), "application/json"),
        )
        .mount(&server)
        .await;

    let adapter = AlchemyEnsResolver::new(rpc(&server.uri()));
    let vitalik = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    let err = adapter
        .reverse(vitalik, Chain::Ethereum)
        .await
        .expect_err("429 must bubble as a domain error");
    assert!(
        matches!(
            err,
            blockexplorer_tui::domain::DomainError::ProviderUnavailable
        ),
        "unexpected error: {err:?}",
    );
}
