//! Functional tests for `load_contract_overview`.
//!
//! See `plan/7-contract-detail.md` section 12.1.

use blockexplorer_tui::{
    application::use_cases::load_contract_overview,
    domain::{
        Address, AddressKind, AddressOverview, Chain, DomainError, ProxyInfo, ProxyKind,
        Wei,
    },
};
use pretty_assertions::assert_eq;

use crate::support::stubs::{StubAddressReaderPort, StubProxyDetectionPort};

fn sample_contract(hex: &str) -> AddressOverview {
    AddressOverview {
        chain: Chain::Ethereum,
        address: Address::from_hex(hex).unwrap(),
        balance: Wei::new(0),
        nonce: 1,
        kind: AddressKind::Contract,
        ens_name: None,
    }
}

#[tokio::test]
async fn contract_without_proxy() {
    let addr_reader = StubAddressReaderPort::new();
    let proxy = StubProxyDetectionPort::new();
    let ov = sample_contract("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    addr_reader.insert(ov.clone());

    let got = load_contract_overview::run(&addr_reader, &proxy, ov.address, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got.account, ov);
    assert!(got.proxy.is_none());
}

#[tokio::test]
async fn contract_with_eip1967_proxy() {
    let addr_reader = StubAddressReaderPort::new();
    let proxy = StubProxyDetectionPort::new();
    let ov = sample_contract("0xa0a1000000000000000000000000000000000001");
    let impl_addr = Address::from_hex("0xb0b1000000000000000000000000000000000002").unwrap();
    addr_reader.insert(ov.clone());
    proxy.set(
        ov.address,
        ProxyInfo {
            kind: ProxyKind::Eip1967,
            implementation: impl_addr,
        },
    );

    let got = load_contract_overview::run(&addr_reader, &proxy, ov.address, Chain::Ethereum)
        .await
        .expect("ok");

    let info = got.proxy.expect("proxy info present");
    assert_eq!(info.kind, ProxyKind::Eip1967);
    assert_eq!(info.implementation, impl_addr);
}

#[tokio::test]
async fn missing_address_returns_not_found() {
    let addr_reader = StubAddressReaderPort::new();
    let proxy = StubProxyDetectionPort::new();
    let addr = Address::from_hex("0x0000000000000000000000000000000000000001").unwrap();

    let err = load_contract_overview::run(&addr_reader, &proxy, addr, Chain::Ethereum)
        .await
        .expect_err("missing account must error");

    assert!(matches!(err, DomainError::NotFound));
}
