//! Functional tests for `CompositeProxyDetector`.
//!
//! See `plan/7-contract-detail.md` section 12.5.1.

use std::sync::{Arc, Mutex};

use blockexplorer_tui::{
    adapters::rpc::CompositeProxyDetector,
    application::ports::{EtherscanProxyHintPort, ProxyDetectionPort},
    domain::{Address, Chain, DomainError, ProxyInfo, ProxyKind, ProxySource},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::StubProxyDetectionPort;

#[derive(Clone, Default)]
struct StubHint {
    by_address: Arc<Mutex<std::collections::HashMap<Address, Option<Address>>>>,
    forced_error: Arc<Mutex<Option<DomainError>>>,
    call_count: Arc<Mutex<usize>>,
}

impl StubHint {
    fn new() -> Self {
        Self::default()
    }

    fn set(&self, address: Address, hint: Option<Address>) {
        self.by_address.lock().unwrap().insert(address, hint);
    }

    fn fail_with(&self, err: DomainError) {
        *self.forced_error.lock().unwrap() = Some(err);
    }

    fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }
}

impl EtherscanProxyHintPort for StubHint {
    async fn implementation_hint(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<Option<Address>, DomainError> {
        *self.call_count.lock().unwrap() += 1;
        if let Some(err) = self.forced_error.lock().unwrap().take() {
            return Err(err);
        }
        Ok(self
            .by_address
            .lock()
            .unwrap()
            .get(&address)
            .copied()
            .unwrap_or(None))
    }
}

fn proxy_addr() -> Address {
    Address::from_hex("0xe6a537a407488807f0bbeb0038b79004f19dddfb").unwrap()
}

fn impl_addr() -> Address {
    Address::from_hex("0xb0b1000000000000000000000000000000000099").unwrap()
}

#[tokio::test]
async fn primary_hit_short_circuits_before_the_hint() {
    let primary = StubProxyDetectionPort::new();
    let hint = StubHint::new();

    let proxy = proxy_addr();
    let implementation = Address::from_hex("0x1111222233334444555566667777888899990000").unwrap();
    primary.set(proxy, ProxyInfo::eip1967_slot(implementation));
    // Even if the hint were primed, it must never be consulted.
    hint.set(proxy, Some(impl_addr()));

    let composite = CompositeProxyDetector::new(primary, Some(hint.clone()));
    let info = composite
        .detect(proxy, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("proxy detected");

    assert_eq!(info.kind, ProxyKind::Eip1967);
    assert_eq!(info.source, ProxySource::Eip1967Slot);
    assert_eq!(info.implementation, implementation);
    assert_eq!(hint.call_count(), 0);
}

#[tokio::test]
async fn primary_miss_falls_back_to_etherscan_hint() {
    let primary = StubProxyDetectionPort::new();
    let hint = StubHint::new();

    let proxy = proxy_addr();
    // Primary is empty; hint points to impl.
    hint.set(proxy, Some(impl_addr()));

    let composite = CompositeProxyDetector::new(primary, Some(hint.clone()));
    let info = composite
        .detect(proxy, Chain::Ethereum)
        .await
        .expect("ok")
        .expect("hint fallback");

    assert_eq!(info.kind, ProxyKind::Eip1967);
    assert_eq!(info.source, ProxySource::EtherscanHint);
    assert_eq!(info.implementation, impl_addr());
    assert_eq!(hint.call_count(), 1);
}

#[tokio::test]
async fn both_miss_returns_none() {
    let primary = StubProxyDetectionPort::new();
    let hint = StubHint::new();
    let composite = CompositeProxyDetector::new(primary, Some(hint));

    let got = composite
        .detect(proxy_addr(), Chain::Ethereum)
        .await
        .expect("ok");
    assert!(got.is_none());
}

#[tokio::test]
async fn hint_error_is_swallowed_and_surfaces_as_no_proxy() {
    // Etherscan outages must never break Contract Detail: if the
    // hint errors, fall through to `None` rather than bubbling the
    // error upwards.
    let primary = StubProxyDetectionPort::new();
    let hint = StubHint::new();
    hint.fail_with(DomainError::ProviderUnavailable);

    let composite = CompositeProxyDetector::new(primary, Some(hint));
    let got = composite
        .detect(proxy_addr(), Chain::Ethereum)
        .await
        .expect("composite must not propagate hint errors");
    assert!(got.is_none());
}

#[tokio::test]
async fn no_hint_port_keeps_primary_semantics() {
    let primary = StubProxyDetectionPort::new();
    let composite: CompositeProxyDetector<_, StubHint> =
        CompositeProxyDetector::primary_only(primary);

    let got = composite
        .detect(proxy_addr(), Chain::Ethereum)
        .await
        .expect("ok");
    assert!(got.is_none());
}
