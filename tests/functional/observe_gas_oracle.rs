//! Functional tests for the `observe_gas_oracle` use case.
//!
//! See `plan/1-home.md` section 4.2.

use blockexplorer_tui::{
    application::use_cases::observe_gas_oracle,
    domain::{Chain, DomainError},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::{GasSnapshotFixture, StubGasOraclePort};

#[tokio::test]
async fn returns_the_primed_snapshot_for_the_requested_chain() {
    let port = StubGasOraclePort::new();
    let primed = GasSnapshotFixture::load("home__gas_snapshot__ethereum.json");
    port.set_snapshot(primed.clone());

    let got = observe_gas_oracle::run(&port, Chain::Ethereum)
        .await
        .expect("snapshot available");

    assert_eq!(got, primed);
}

#[tokio::test]
async fn reports_provider_unavailable_when_stub_is_broken() {
    let port = StubGasOraclePort::new();
    port.set_broken(true);

    let err = observe_gas_oracle::run(&port, Chain::Ethereum)
        .await
        .expect_err("broken stub must error out");

    assert!(matches!(err, DomainError::ProviderUnavailable));
}
