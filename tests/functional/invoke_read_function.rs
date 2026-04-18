//! Functional tests for the `invoke_read_function` use case.
//!
//! See `plan/7-contract-detail.md` section 12.4.2.

use blockexplorer_tui::{
    application::use_cases::invoke_read_function,
    domain::{
        AbiFunction, AbiParam, AbiParamType, AbiValue, Address, Chain, DecodedValue, DomainError,
    },
};
use pretty_assertions::assert_eq;

use crate::support::stubs::StubContractReaderPort;

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
async fn executes_a_view_function_and_returns_decoded_values() {
    let reader = StubContractReaderPort::new();
    let contract =
        Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    let function = balance_of();
    reader.set_result(
        contract,
        &function.signature(),
        vec![DecodedValue::Uint(1_000_000)],
    );

    let owner = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    let result = invoke_read_function::run(
        &reader,
        contract,
        Chain::Ethereum,
        &function,
        vec![AbiValue::Address(owner)],
    )
    .await
    .expect("ok");

    assert_eq!(result, vec![DecodedValue::Uint(1_000_000)]);
}

#[tokio::test]
async fn surfaces_revert_reason_on_execution_revert() {
    let reader = StubContractReaderPort::new();
    let contract =
        Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    let function = balance_of();
    reader.set_revert(contract, &function.signature(), "InsufficientBalance()");

    let owner = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    let err = invoke_read_function::run(
        &reader,
        contract,
        Chain::Ethereum,
        &function,
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

#[tokio::test]
async fn rejects_non_read_only_functions_upfront() {
    let reader = StubContractReaderPort::new();
    let contract =
        Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    let mut function = balance_of();
    function.is_read_only = false;

    let owner = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    let err = invoke_read_function::run(
        &reader,
        contract,
        Chain::Ethereum,
        &function,
        vec![AbiValue::Address(owner)],
    )
    .await
    .expect_err("should refuse non-view");
    assert!(matches!(err, DomainError::InvalidInput(_)));
}

#[tokio::test]
async fn rejects_arg_count_mismatch() {
    let reader = StubContractReaderPort::new();
    let contract =
        Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    let function = balance_of();

    let err = invoke_read_function::run(
        &reader,
        contract,
        Chain::Ethereum,
        &function,
        vec![], // expected 1, got 0
    )
    .await
    .expect_err("should refuse arg mismatch");
    assert!(matches!(err, DomainError::InvalidInput(_)));
}
