//! Functional tests for `HomeSession`.
//!
//! See `plan/1-home.md` section 11.3.

use blockexplorer_tui::{
    application::{ConnectionStatus, HomeSession},
    domain::Chain,
};
use pretty_assertions::assert_eq;

use crate::support::stubs::{
    GasSnapshotFixture, NetworkStatusFixture, StubChainRegistry, StubGasOraclePort,
    StubNetworkStatusPort,
};

fn primed_session() -> HomeSession<StubNetworkStatusPort, StubGasOraclePort, StubChainRegistry> {
    let network = StubNetworkStatusPort::new();
    let gas = StubGasOraclePort::new();
    let chains = StubChainRegistry::with_all_enabled();

    network.set_snapshot(NetworkStatusFixture::load(
        "home__network_status__ethereum.json",
    ));
    gas.set_snapshot(GasSnapshotFixture::load(
        "home__gas_snapshot__ethereum.json",
    ));
    network.set_snapshot(NetworkStatusFixture::load(
        "home__network_status__base.json",
    ));
    gas.set_snapshot(GasSnapshotFixture::load("home__gas_snapshot__base.json"));

    HomeSession::new(network, gas, chains, Chain::Ethereum)
}

#[tokio::test]
async fn refresh_populates_network_and_gas_view() {
    let mut session = primed_session();

    session.refresh().await.expect("refresh ok");
    let view = session.view();

    assert_eq!(view.chain, Chain::Ethereum);
    assert!(view.network.is_some());
    assert!(view.gas.is_some());
    assert_eq!(view.connection, ConnectionStatus::Connected);
}

#[tokio::test]
async fn on_new_head_updates_both_cards() {
    let mut session = primed_session();
    session.refresh().await.unwrap();

    // Advance the fixtures to the "next head" values.
    let network = StubNetworkStatusPort::new();
    let gas = StubGasOraclePort::new();
    network.set_snapshot(NetworkStatusFixture::load(
        "home__network_status__ethereum_next_head.json",
    ));
    gas.set_snapshot(GasSnapshotFixture::load(
        "home__gas_snapshot__ethereum_next_head.json",
    ));
    let chains = StubChainRegistry::with_all_enabled();
    let mut session = HomeSession::new(network, gas, chains, Chain::Ethereum);

    session.on_new_head().await.expect("refresh ok");
    let view = session.view();

    assert_eq!(
        view.network.as_ref().unwrap().latest_block.value(),
        21_345_679
    );
    assert_eq!(view.gas.as_ref().unwrap().average.value(), 17);
}

#[tokio::test]
async fn switch_chain_swaps_the_active_chain_and_refreshes() {
    let mut session = primed_session();
    session.refresh().await.unwrap();

    session.switch_chain(Chain::Base).await.expect("switch ok");
    let view = session.view();

    assert_eq!(view.chain, Chain::Base);
    assert_eq!(view.network.as_ref().unwrap().chain, Chain::Base);
    assert_eq!(view.gas.as_ref().unwrap().chain, Chain::Base);
}

#[tokio::test]
async fn switch_chain_rejects_disabled_chain() {
    let network = StubNetworkStatusPort::new();
    let gas = StubGasOraclePort::new();
    network.set_snapshot(NetworkStatusFixture::load(
        "home__network_status__ethereum.json",
    ));
    gas.set_snapshot(GasSnapshotFixture::load(
        "home__gas_snapshot__ethereum.json",
    ));
    let chains = StubChainRegistry::new(vec![Chain::Ethereum], Chain::Ethereum);
    let mut session = HomeSession::new(network, gas, chains, Chain::Ethereum);
    session.refresh().await.unwrap();

    let err = session
        .switch_chain(Chain::Base)
        .await
        .expect_err("disabled chain must be rejected");

    use blockexplorer_tui::domain::DomainError;
    assert!(matches!(err, DomainError::FeatureUnavailable));
    assert_eq!(session.view().chain, Chain::Ethereum);
}

#[tokio::test]
async fn connection_drop_flips_view_model_flag() {
    let mut session = primed_session();
    session.refresh().await.unwrap();

    session.on_connection_drop();
    let view = session.view();

    assert!(matches!(
        view.connection,
        ConnectionStatus::Disconnected {
            reconnect_scheduled: true
        }
    ));
}

#[tokio::test]
async fn refresh_with_broken_provider_marks_disconnected() {
    let network = StubNetworkStatusPort::new();
    let gas = StubGasOraclePort::new();
    let chains = StubChainRegistry::with_all_enabled();
    network.set_snapshot(NetworkStatusFixture::load(
        "home__network_status__ethereum.json",
    ));
    gas.set_snapshot(GasSnapshotFixture::load(
        "home__gas_snapshot__ethereum.json",
    ));
    network.set_broken(true);

    let mut session = HomeSession::new(network, gas, chains, Chain::Ethereum);

    session.refresh().await.expect("should not propagate error");
    assert!(matches!(
        session.view().connection,
        ConnectionStatus::Disconnected {
            reconnect_scheduled: true
        }
    ));
}
