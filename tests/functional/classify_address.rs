//! Functional tests for the pure `classify_address` helper.
//!
//! See `plan/15-backlog.md` section 3.1 and 6.

use blockexplorer_tui::{
    application::use_cases::classify_address,
    domain::{Address, AddressKind, DomainError},
};
use pretty_assertions::assert_eq;
use rstest::rstest;

#[test]
fn it_classifies_delegated_eoa_as_eoa_with_delegate() {
    let delegate_hex = "c0ffee000000000000000000000000000000babe";
    let code = format!("0xef0100{delegate_hex}");

    let kind = classify_address::run(&code).expect("ok");

    let expected_delegate = Address::from_hex(&format!("0x{delegate_hex}")).unwrap();
    assert_eq!(
        kind,
        AddressKind::Eoa {
            delegated_to: Some(expected_delegate),
        },
    );
}

#[rstest]
#[case("0x")]
#[case("0X")]
#[case("0x0000000000")]
fn it_classifies_plain_eoa_as_eoa(#[case] code: &str) {
    let kind = classify_address::run(code).expect("ok");
    assert_eq!(kind, AddressKind::Eoa { delegated_to: None });
}

#[test]
fn it_classifies_contract_as_contract() {
    let code = "0x608060405234801561001057600080fd5b5061002a6104ec";
    let kind = classify_address::run(code).expect("ok");
    assert_eq!(kind, AddressKind::Contract);
}

#[test]
fn it_treats_ef0100_without_full_delegate_as_contract() {
    // A designator prefix that is not followed by a complete 20-byte
    // delegate is not a valid EIP-7702 tag. Fall back to contract so
    // we do not silently hide bytecode.
    let code = "0xef0100dead";
    let kind = classify_address::run(code).expect("ok");
    assert_eq!(kind, AddressKind::Contract);
}

#[rstest]
#[case("not-hex-at-all")]
#[case("0xNOTHEX")]
#[case("0xabc")]
fn it_rejects_invalid_code_response(#[case] code: &str) {
    let err = classify_address::run(code).expect_err("invalid code must error");
    assert!(matches!(err, DomainError::Internal(_)));
}
