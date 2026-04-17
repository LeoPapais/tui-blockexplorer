//! Functional tests for the `observe_network_status` use case.
//!
//! See `plan/1-home.md` section 4.1.

use blockexplorer_tui::{
    application::use_cases::observe_network_status,
    domain::{Chain, DomainError},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::{NetworkStatusFixture, StubNetworkStatusPort};

#[tokio::test]
async fn returns_the_primed_snapshot_for_the_requested_chain() {
    let port = StubNetworkStatusPort::new();
    let primed = NetworkStatusFixture::load("home__network_status__ethereum.json");
    port.set_snapshot(primed.clone());

    let got = observe_network_status::run(&port, Chain::Ethereum)
        .await
        .expect("snapshot available");

    assert_eq!(got, primed);
}

#[tokio::test]
async fn reports_provider_unavailable_when_stub_is_broken() {
    let port = StubNetworkStatusPort::new();
    port.set_broken(true);

    let err = observe_network_status::run(&port, Chain::Ethereum)
        .await
        .expect_err("broken stub must error out");

    assert!(matches!(err, DomainError::ProviderUnavailable));
}

#[tokio::test]
async fn returns_not_found_when_chain_has_no_fixture() {
    let port = StubNetworkStatusPort::new();

    let err = observe_network_status::run(&port, Chain::Ethereum)
        .await
        .expect_err("unprimed chain must error out");

    assert!(matches!(err, DomainError::NotFound));
}
