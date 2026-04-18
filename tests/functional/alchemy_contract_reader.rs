//! Wiremock tests for the Alchemy `eth_call` ABI reader.
//!
//! Drives [`AlchemyContractReader`] end to end: argument encoding,
//! transport, decoding and revert handling.
//!
//! See `plan/7-contract-detail.md` section 12.4.2.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyContractReader, RpcClient},
    application::ports::ContractReaderPort,
    domain::{
        AbiFunction, AbiParam, AbiParamType, AbiValue, Address, Chain, DecodedValue,
        DomainError,
    },
};
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::method,
};

use crate::support::fixture_loader::load_text;

fn adapter_for(url: &str) -> AlchemyContractReader {
    let rpc = RpcClient::new(Url::parse(url).unwrap(), reqwest::Client::new());
    AlchemyContractReader::new(rpc)
}

fn balance_of() -> AbiFunction {
    AbiFunction {
        name: "balanceOf".into(),
        inputs: vec![AbiParam {
            name: "owner".into(),
            kind: AbiParamType::Address,
        }],
        outputs: vec![AbiParam {
            name: "".into(),
            kind: AbiParamType::Uint { bits: 256 },
        }],
        is_read_only: true,
    }
}

#[tokio::test]
async fn decodes_a_uint_return_value() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("alchemy__eth_call__balanceOf_success.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let contract = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    let owner = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

    let result = adapter
        .call(
            contract,
            Chain::Ethereum,
            &balance_of(),
            vec![AbiValue::Address(owner)],
        )
        .await
        .expect("ok");

    assert_eq!(result, vec![DecodedValue::Uint(1_000_000)]);
}

#[tokio::test]
async fn maps_revert_error_to_execution_reverted() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("alchemy__eth_call__revert_insufficient.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let contract = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    let owner = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();

    let err = adapter
        .call(
            contract,
            Chain::Ethereum,
            &balance_of(),
            vec![AbiValue::Address(owner)],
        )
        .await
        .expect_err("revert expected");

    match err {
        DomainError::ExecutionReverted { reason } => {
            assert_eq!(reason, "InsufficientBalance()");
        }
        other => panic!("expected ExecutionReverted, got {other:?}"),
    }
}
