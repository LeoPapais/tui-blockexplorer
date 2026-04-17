//! Functional tests for `load_address_overview`.
//!
//! See `plan/6-address-detail.md` section 12.1.

use blockexplorer_tui::{
    application::use_cases::load_address_overview,
    domain::{Address, AddressKind, AddressOverview, Chain, DomainError, Wei},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::StubAddressReaderPort;

fn sample(kind: AddressKind, hex: &str) -> AddressOverview {
    AddressOverview {
        chain: Chain::Ethereum,
        address: Address::from_hex(hex).unwrap(),
        balance: Wei::new(523_140_000_000_000_000_000u128),
        nonce: 1_243,
        kind,
        ens_name: None,
    }
}

#[tokio::test]
async fn happy_path_for_an_eoa() {
    let reader = StubAddressReaderPort::new();
    let ov = sample(AddressKind::Eoa, "0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
    reader.insert(ov.clone());

    let got = load_address_overview::run(&reader, ov.address, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got, ov);
    assert_eq!(got.kind, AddressKind::Eoa);
}

#[tokio::test]
async fn happy_path_for_a_contract() {
    let reader = StubAddressReaderPort::new();
    let ov = sample(AddressKind::Contract, "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    reader.insert(ov.clone());

    let got = load_address_overview::run(&reader, ov.address, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got.kind, AddressKind::Contract);
}

#[tokio::test]
async fn missing_address_returns_not_found() {
    let reader = StubAddressReaderPort::new();
    let addr = Address::from_hex("0x0000000000000000000000000000000000000001").unwrap();

    let err = load_address_overview::run(&reader, addr, Chain::Ethereum)
        .await
        .expect_err("missing address must error");

    assert!(matches!(err, DomainError::NotFound));
}
