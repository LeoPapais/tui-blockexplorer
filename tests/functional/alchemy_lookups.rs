//! Wiremock-driven tests for the Search-related Alchemy adapters:
//! `AlchemyTxLookup`, `AlchemyBlockLookup`, `AlchemyAddressLookup` and
//! `AlchemyEnsResolver`.
//!
//! See `plan/2-search.md` section 10.2.

use blockexplorer_tui::{
    adapters::rpc::{
        AlchemyAddressLookup, AlchemyBlockLookup, AlchemyEnsResolver, AlchemyTxLookup, RpcClient,
    },
    application::ports::{AddressLookupPort, BlockLookupPort, EnsResolverPort, TxLookupPort},
    domain::{Address, AddressKind, BlockHash, BlockNumber, Chain, TxHash},
};
use serde_json::json;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

use crate::support::fixture_loader::load_text;

fn rpc(url: &str) -> RpcClient {
    RpcClient::new(Url::parse(url).unwrap(), reqwest::Client::new())
}

// ---------------------------------------------------------------------------
// AlchemyTxLookup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tx_lookup_returns_summary_when_found() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(
            json!({"method":"eth_getTransactionByHash"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getTransactionByHash__found.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = AlchemyTxLookup::new(rpc(&server.uri()));
    let hash =
        TxHash::from_hex("0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b")
            .unwrap();

    let got = adapter.get(hash, Chain::Ethereum).await.expect("ok");
    let summary = got.expect("found");
    assert_eq!(summary.hash, hash);
    assert_eq!(summary.block, Some(BlockNumber::new(21_000_000)));
}

#[tokio::test]
async fn tx_lookup_returns_none_when_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(
            json!({"method":"eth_getTransactionByHash"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getTransactionByHash__not_found.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = AlchemyTxLookup::new(rpc(&server.uri()));
    let hash =
        TxHash::from_hex("0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b")
            .unwrap();

    let got = adapter.get(hash, Chain::Ethereum).await.expect("ok");
    assert!(got.is_none());
}

// ---------------------------------------------------------------------------
// AlchemyBlockLookup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn block_lookup_by_hash_returns_summary() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockByHash"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBlockByHash__found.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = AlchemyBlockLookup::new(rpc(&server.uri()));
    let hash =
        BlockHash::from_hex("0xabcdef0000000000000000000000000000000000000000000000000000000000")
            .unwrap();

    let summary = adapter
        .get_by_hash(hash, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("found");
    assert_eq!(summary.number.value(), 21_000_000);
}

#[tokio::test]
async fn block_lookup_by_hash_returns_none_when_missing() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockByHash"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBlockByHash__not_found.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = AlchemyBlockLookup::new(rpc(&server.uri()));
    let hash =
        BlockHash::from_hex("0xabcdef0000000000000000000000000000000000000000000000000000000000")
            .unwrap();

    let got = adapter
        .get_by_hash(hash, Chain::Ethereum)
        .await
        .expect("ok");
    assert!(got.is_none());
}

#[tokio::test]
async fn block_lookup_by_number_returns_summary() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getBlockByNumber"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getBlockByHash__found.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = AlchemyBlockLookup::new(rpc(&server.uri()));
    let summary = adapter
        .get_by_number(BlockNumber::new(21_000_000), Chain::Ethereum)
        .await
        .expect("ok")
        .expect("found");
    assert_eq!(summary.number.value(), 21_000_000);
}

// ---------------------------------------------------------------------------
// AlchemyAddressLookup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn address_lookup_reports_eoa_when_code_is_empty() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getCode"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(load_text("rpc__eth_getCode__eoa.json"), "application/json"),
        )
        .mount(&server)
        .await;

    let adapter = AlchemyAddressLookup::new(rpc(&server.uri()));
    let addr = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

    let kind = adapter.classify(addr, Chain::Ethereum).await.expect("ok");
    assert_eq!(kind, AddressKind::Eoa { delegated_to: None });
}

#[tokio::test]
async fn address_lookup_reports_contract_when_code_is_non_empty() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getCode"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getCode__contract.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = AlchemyAddressLookup::new(rpc(&server.uri()));
    let addr = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();

    let kind = adapter.classify(addr, Chain::Ethereum).await.expect("ok");
    assert_eq!(kind, AddressKind::Contract);
}

#[tokio::test]
async fn address_lookup_reports_delegated_eoa_when_code_starts_with_ef0100() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"eth_getCode"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_getCode__delegated_eoa.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = AlchemyAddressLookup::new(rpc(&server.uri()));
    let addr = Address::from_hex("0x5abc0e99dfc7ba2c9da42f8dc91ec4128a89e919").unwrap();

    let kind = adapter.classify(addr, Chain::Ethereum).await.expect("ok");
    let delegate = Address::from_hex("0xc0ffee000000000000000000000000000000babe").unwrap();
    assert_eq!(
        kind,
        AddressKind::Eoa {
            delegated_to: Some(delegate),
        },
    );
}

// ---------------------------------------------------------------------------
// AlchemyEnsResolver
// ---------------------------------------------------------------------------
//
// Forward resolution issues two eth_call posts; wiremock answers both
// by returning the next canned response in order thanks to partial JSON
// matching by `data` prefix.

#[tokio::test]
async fn ens_forward_resolves_a_name_via_two_eth_calls() {
    let server = MockServer::start().await;
    // The first eth_call hits the Registry (resolver(bytes32)) and
    // returns the resolver address. The "data" field starts with the
    // selector 0x0178b8bf.
    Mock::given(method("POST"))
        .and(body_partial_json(json!({
            "method":"eth_call",
            "params": [{"to":"0x00000000000c2e074ec69a0dfb2997ba6c7d2e1e"}, "latest"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_call__ens_resolver.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    // The second eth_call goes to the resolver contract returned above
    // (0x4976...). Matching by `to` keeps it unambiguous.
    Mock::given(method("POST"))
        .and(body_partial_json(json!({
            "method":"eth_call",
            "params": [{"to":"0x4976fb03c32e5b8cfe2b6ccb31c09ba78ebaba41"}, "latest"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_call__ens_addr.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = AlchemyEnsResolver::new(rpc(&server.uri()));
    let addr = adapter
        .forward("vitalik.eth", Chain::Ethereum)
        .await
        .expect("ok")
        .expect("resolved");

    assert_eq!(addr.to_hex(), "0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
}

#[tokio::test]
async fn ens_forward_returns_none_when_resolver_is_zero() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({
            "method":"eth_call",
            "params": [{"to":"0x00000000000c2e074ec69a0dfb2997ba6c7d2e1e"}, "latest"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("rpc__eth_call__ens_resolver_none.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = AlchemyEnsResolver::new(rpc(&server.uri()));
    let got = adapter
        .forward("does-not-exist.eth", Chain::Ethereum)
        .await
        .expect("ok");
    assert!(got.is_none());
}
