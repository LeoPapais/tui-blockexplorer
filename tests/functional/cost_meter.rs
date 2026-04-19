//! Functional test: `RpcClient` bumps the installed `CostRecorder`
//! with the hint returned by `cost_hint_for` on every successful
//! call. See `plan/13-alchemy-adapter.md` §8.4 and
//! `plan/15-backlog.md` §8.14 item 5.

use std::sync::Arc;

use blockexplorer_tui::{
    adapters::rpc::{CostRecorder, RpcClient, cost_hint_for},
    infra::cost_meter::CostMeter,
};
use pretty_assertions::assert_eq;
use serde_json::json;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

use crate::support::fixture_loader::load_text;

#[tokio::test]
async fn successful_call_charges_cost_hint_into_meter() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method": "eth_blockNumber"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_blockNumber__ethereum.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let meter: Arc<CostMeter> = CostMeter::shared();
    let recorder: Arc<dyn CostRecorder> = meter.clone();
    let client = RpcClient::new(Url::parse(&server.uri()).unwrap(), reqwest::Client::new())
        .with_cost_recorder(recorder);

    let _value: String = client
        .call("eth_blockNumber", json!([]))
        .await
        .expect("ok");

    assert_eq!(
        meter.consumed(),
        u64::from(cost_hint_for("eth_blockNumber").compute_units),
    );
}

#[tokio::test]
async fn failed_calls_do_not_charge_the_meter() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__error__invalid_params.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let meter = CostMeter::shared();
    let recorder: Arc<dyn CostRecorder> = meter.clone();
    let client = RpcClient::new(Url::parse(&server.uri()).unwrap(), reqwest::Client::new())
        .with_cost_recorder(recorder);

    let err = client
        .call::<_, String>("eth_getTransactionByHash", json!([]))
        .await
        .expect_err("failed call");
    let _ = err;

    assert_eq!(
        meter.consumed(),
        0,
        "failures must not burn CU budget in the meter",
    );
}

#[tokio::test]
async fn unknown_methods_are_charged_the_fallback_weight() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"jsonrpc":"2.0","id":1,"result":"0x1"}"#,
            "application/json",
        ))
        .mount(&server)
        .await;

    let meter = CostMeter::shared();
    let recorder: Arc<dyn CostRecorder> = meter.clone();
    let client = RpcClient::new(Url::parse(&server.uri()).unwrap(), reqwest::Client::new())
        .with_cost_recorder(recorder);

    let _value: String = client
        .call("totally_not_a_real_method", json!([]))
        .await
        .expect("ok");

    assert_eq!(
        meter.consumed(),
        u64::from(cost_hint_for("totally_not_a_real_method").compute_units),
    );
}
