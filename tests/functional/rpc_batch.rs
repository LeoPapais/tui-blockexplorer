//! Functional tests for `RpcClient::call_batch`.
//!
//! See `plan/13-alchemy-adapter.md` §8.5 and
//! `plan/15-backlog.md` §8.14 item 4. The tests exercise the
//! end-to-end envelope: JSON array request body, per-entry result
//! matching by `id`, and the degenerate empty-call path that must
//! not touch the network.

use blockexplorer_tui::adapters::rpc::{RpcClient, RpcError};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use url::Url;
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate, matchers::method};

struct EchoBatchResponder;

impl Respond for EchoBatchResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        // Panic surfaces as a wiremock 500 in the client; the test
        // fails for the right reason.
        let Value::Array(envelopes) = body else {
            return ResponseTemplate::new(500).set_body_string("expected a JSON array body");
        };
        let responses: Vec<Value> = envelopes
            .iter()
            .map(|env| {
                let id = env.get("id").cloned().unwrap_or(Value::Null);
                let method = env
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": format!("echo:{method}"),
                })
            })
            .collect();
        ResponseTemplate::new(200).set_body_json(Value::Array(responses))
    }
}

#[tokio::test]
async fn call_batch_posts_array_and_returns_results_in_request_order() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(EchoBatchResponder)
        .mount(&server)
        .await;
    let client = RpcClient::new(Url::parse(&server.uri()).unwrap(), reqwest::Client::new());

    let params: Vec<[String; 1]> = vec![["0x1".into()], ["0x2".into()], ["0x3".into()]];
    let out = client
        .call_batch::<_, String>("my_method", &params)
        .await
        .expect("batch envelope parses");

    assert_eq!(out.len(), 3);
    for (i, result) in out.iter().enumerate() {
        let value = result.as_ref().expect("per-call ok");
        assert_eq!(value, "echo:my_method", "row {i} must be in request order");
    }
}

#[tokio::test]
async fn call_batch_matches_results_by_id_when_provider_reorders() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(|request: &Request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
            let Value::Array(envelopes) = body else {
                return ResponseTemplate::new(500);
            };
            let mut responses: Vec<Value> = envelopes
                .iter()
                .map(|env| {
                    json!({
                        "jsonrpc": "2.0",
                        "id": env.get("id").cloned().unwrap_or(Value::Null),
                        "result": env.get("params").and_then(|p| p.get(0)).cloned().unwrap_or(Value::Null),
                    })
                })
                .collect();
            // Reverse the order so id=1 lands at the end.
            responses.reverse();
            ResponseTemplate::new(200).set_body_json(Value::Array(responses))
        })
        .mount(&server)
        .await;
    let client = RpcClient::new(Url::parse(&server.uri()).unwrap(), reqwest::Client::new());

    let params: Vec<[String; 1]> = vec![["A".into()], ["B".into()], ["C".into()]];
    let out = client
        .call_batch::<_, String>("whatever", &params)
        .await
        .expect("ok");

    // After `id`-based reassembly we still see request order.
    assert_eq!(out[0].as_ref().unwrap(), "A");
    assert_eq!(out[1].as_ref().unwrap(), "B");
    assert_eq!(out[2].as_ref().unwrap(), "C");
}

#[tokio::test]
async fn call_batch_surfaces_per_entry_rpc_errors_without_failing_the_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(|_req: &Request| {
            ResponseTemplate::new(200).set_body_json(json!([
                { "jsonrpc": "2.0", "id": 1, "result": "ok" },
                { "jsonrpc": "2.0", "id": 2, "error": { "code": -32602, "message": "invalid" } },
            ]))
        })
        .mount(&server)
        .await;
    let client = RpcClient::new(Url::parse(&server.uri()).unwrap(), reqwest::Client::new());

    let out = client
        .call_batch::<_, String>("x", &[["a".to_string()], ["b".to_string()]])
        .await
        .expect("envelope ok");
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].as_ref().unwrap(), "ok");
    assert!(matches!(
        out[1].as_ref().unwrap_err(),
        RpcError::Rpc { code: -32602, .. }
    ));
}

#[tokio::test]
async fn empty_calls_short_circuits_without_touching_the_network() {
    // Bind wiremock but register no matcher — any request would 404
    // and bubble as an HTTP 5xx-like failure. The test proves we
    // never reach the mock.
    let server = MockServer::start().await;
    let client = RpcClient::new(Url::parse(&server.uri()).unwrap(), reqwest::Client::new());
    let params: Vec<[String; 1]> = Vec::new();
    let out = client
        .call_batch::<_, String>("x", &params)
        .await
        .expect("empty batch is a no-op");
    assert!(out.is_empty());
    assert_eq!(server.received_requests().await.unwrap().len(), 0);
}
