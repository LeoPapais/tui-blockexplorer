//! Wiremock tests for the Alchemy portfolio adapter.
//!
//! Drives `alchemy_getTokenBalances` + per-holding
//! `alchemy_getTokenMetadata`. Zero-balance rows are filtered and
//! the metadata fan-out is bounded.
//!
//! See `plan/6-address-detail.md` section 12.4.2.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyPortfolio, RpcClient},
    application::ports::PortfolioPort,
    domain::{Address, Chain},
};
use serde_json::Value;
use url::Url;
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate, matchers::method};

use crate::support::fixture_loader::load_text;

struct AlchemyResponder;

impl Respond for AlchemyResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        let method = body
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let first_param = body
            .get("params")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();

        let fixture = match method {
            "alchemy_getTokenBalances" => "alchemy__token_balances.json",
            "alchemy_getTokenMetadata" => {
                if first_param.starts_with("0xa0b86991") {
                    "alchemy__token_metadata_usdc.json"
                } else {
                    "alchemy__token_metadata_usdt.json"
                }
            }
            _ => "alchemy__token_balances.json",
        };
        ResponseTemplate::new(200).set_body_raw(load_text(fixture), "application/json")
    }
}

fn adapter_for(url: &str) -> AlchemyPortfolio {
    let rpc = RpcClient::new(Url::parse(url).unwrap(), reqwest::Client::new());
    AlchemyPortfolio::new(rpc)
}

#[tokio::test]
async fn merges_balances_with_metadata_descending() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(AlchemyResponder)
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let address = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

    let holdings = adapter
        .get_token_balances(address, Chain::Ethereum)
        .await
        .expect("ok");

    // Two non-zero rows after the zero-balance entry is filtered.
    assert_eq!(holdings.len(), 2);
    // Sorted descending by raw balance: USDC (1_000_000) > USDT (10_000).
    assert_eq!(holdings[0].metadata.symbol, "USDC");
    assert_eq!(holdings[0].metadata.decimals, 6);
    assert_eq!(holdings[0].balance.value(), 1_000_000);
    assert_eq!(holdings[1].metadata.symbol, "USDT");
    assert_eq!(holdings[1].balance.value(), 10_000);
}

#[tokio::test]
async fn empty_response_yields_empty_holdings() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"jsonrpc":"2.0","id":1,"result":{"address":"0x0","tokenBalances":[]}}"#,
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let address = Address::from_hex("0x0000000000000000000000000000000000000099").unwrap();

    let holdings = adapter
        .get_token_balances(address, Chain::Ethereum)
        .await
        .expect("ok");
    assert!(holdings.is_empty());
}
