//! Functional tests for the `ContractSourcePort::get_abi_following_proxy`
//! default method.
//!
//! See `plan/15-backlog.md` section 3.3.

use blockexplorer_tui::{
    application::ports::ContractSourcePort,
    domain::{AbiSource, Address, Chain, ContractAbi, ProxyInfo},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::{StubContractSourcePort, StubProxyDetectionPort};

fn contract_address() -> Address {
    Address::from_hex("0xe6a537a407488807f0bbeb0038b79004f19dddfb").unwrap()
}

fn implementation_address() -> Address {
    Address::from_hex("0x1111222233334444555566667777888899990000").unwrap()
}

fn transfer_abi() -> ContractAbi {
    ContractAbi {
        abi: r#"[{"type":"function","name":"transfer","inputs":[{"name":"to","type":"address"},{"name":"value","type":"uint256"}],"outputs":[{"name":"","type":"bool"}]}]"#
            .to_string(),
        is_verified: true,
    }
}

fn proxy_only_abi() -> ContractAbi {
    ContractAbi {
        abi: r#"[{"type":"function","name":"implementation","inputs":[],"outputs":[{"name":"","type":"address"}]}]"#
            .to_string(),
        is_verified: true,
    }
}

#[tokio::test]
async fn returns_direct_abi_when_get_abi_hits() {
    let source = StubContractSourcePort::new();
    let detector = StubProxyDetectionPort::new();

    let addr = contract_address();
    source.insert(addr, transfer_abi());

    let got = source
        .get_abi_following_proxy(addr, Chain::Ethereum, &detector)
        .await
        .expect("ok")
        .expect("resolved abi");

    assert_eq!(got.source_kind, AbiSource::Direct);
    assert!(got.abi.contains("transfer"));
}

#[tokio::test]
async fn returns_proxy_implementation_when_direct_is_missing() {
    let source = StubContractSourcePort::new();
    let detector = StubProxyDetectionPort::new();

    let proxy = contract_address();
    let implementation = implementation_address();

    // No direct ABI; proxy detection and implementation ABI are primed.
    detector.set(proxy, ProxyInfo::eip1967_slot(implementation));
    source.insert(implementation, transfer_abi());

    let got = source
        .get_abi_following_proxy(proxy, Chain::Ethereum, &detector)
        .await
        .expect("ok")
        .expect("resolved abi");

    match got.source_kind {
        AbiSource::ProxyImplementation {
            proxy: p,
            implementation: i,
        } => {
            assert_eq!(p, proxy);
            assert_eq!(i, implementation);
        }
        other => panic!("expected ProxyImplementation, got {other:?}"),
    }
    assert!(got.abi.contains("transfer"));
}

#[tokio::test]
async fn returns_none_when_proxy_detected_but_implementation_abi_missing() {
    let source = StubContractSourcePort::new();
    let detector = StubProxyDetectionPort::new();

    let proxy = contract_address();
    let implementation = implementation_address();

    detector.set(proxy, ProxyInfo::eip1967_slot(implementation));
    // No ABI primed for either proxy or implementation.

    let got = source
        .get_abi_following_proxy(proxy, Chain::Ethereum, &detector)
        .await
        .expect("ok");

    assert!(got.is_none());
}

#[tokio::test]
async fn returns_none_when_both_direct_and_detection_miss() {
    let source = StubContractSourcePort::new();
    let detector = StubProxyDetectionPort::new();

    let got = source
        .get_abi_following_proxy(contract_address(), Chain::Ethereum, &detector)
        .await
        .expect("ok");

    assert!(got.is_none());
}

#[tokio::test]
async fn direct_abi_wins_even_when_detection_would_otherwise_trigger() {
    // Guard against regressions: the default method must stop after
    // the first direct hit and never call the detector. Here both the
    // direct ABI and the detector are primed; the returned value must
    // surface the direct ABI verbatim, with `AbiSource::Direct`.
    let source = StubContractSourcePort::new();
    let detector = StubProxyDetectionPort::new();

    let proxy = contract_address();
    let implementation = implementation_address();

    source.insert(proxy, proxy_only_abi());
    detector.set(proxy, ProxyInfo::eip1967_slot(implementation));
    source.insert(implementation, transfer_abi());

    let got = source
        .get_abi_following_proxy(proxy, Chain::Ethereum, &detector)
        .await
        .expect("ok")
        .expect("resolved abi");

    assert_eq!(got.source_kind, AbiSource::Direct);
    assert!(got.abi.contains("implementation"));
    assert!(!got.abi.contains("transfer"));
}
