//! Functional tests for the `HomeScreen` feed channel and the
//! background refresher in `src/infra/home_feed.rs`.
//!
//! See `plan/14-config-and-credentials.md` section 3 and
//! `plan/1-home.md` section 12.5.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{HomeScreen, Screen, home_feed},
    application::{ConnectionStatus, HomeSession, HomeViewModel},
    domain::{BlockNumber, Chain, GasSnapshot, Gwei, NetworkStatus, NewHead, Wei},
    infra::home_feed as infra_home_feed,
};
use pretty_assertions::assert_eq;

use crate::support::stubs::{
    GasSnapshotFixture, NetworkStatusFixture, StubChainRegistry, StubGasOraclePort,
    StubNetworkStatusPort, StubNewHeadsStreamPort,
};

fn sample_view(block: u64) -> HomeViewModel {
    HomeViewModel {
        chain: Chain::Ethereum,
        network: Some(NetworkStatus {
            chain: Chain::Ethereum,
            latest_block: BlockNumber::new(block),
            base_fee: Wei::new(1_000_000_000),
            block_time_avg_ms: 12_000,
        }),
        gas: Some(GasSnapshot {
            chain: Chain::Ethereum,
            slow: Gwei::new(10),
            average: Gwei::new(12),
            fast: Gwei::new(15),
            base_fee: Gwei::new(1),
            trend: vec![Gwei::new(1); 20],
        }),
        connection: ConnectionStatus::Connected,
    }
}

#[tokio::test]
async fn tick_drains_feed_into_view_model() {
    let initial = sample_view(1);
    let (feed, sender) = home_feed();
    let mut screen = HomeScreen::with_feed(initial.clone(), feed);

    assert_eq!(
        screen.view().network.as_ref().unwrap().latest_block.value(),
        1
    );

    sender.send(sample_view(2)).expect("feed alive");
    sender.send(sample_view(3)).expect("feed alive");

    screen.tick();

    assert_eq!(
        screen.view().network.as_ref().unwrap().latest_block.value(),
        3,
        "tick must keep the latest update",
    );
}

#[tokio::test]
async fn tick_without_updates_keeps_view() {
    let initial = sample_view(1);
    let (feed, _sender) = home_feed();
    let mut screen = HomeScreen::with_feed(initial.clone(), feed);

    screen.tick();

    assert_eq!(screen.view(), &initial);
}

#[tokio::test]
async fn dropped_sender_disables_further_polls() {
    let initial = sample_view(1);
    let (feed, sender) = home_feed();
    let mut screen = HomeScreen::with_feed(initial.clone(), feed);

    drop(sender);
    screen.tick();
    // A second tick must not panic even though the channel is closed.
    screen.tick();

    assert_eq!(screen.view(), &initial);
}

/// plan/1-home.md §12.5: when a WS event arrives the dispatcher calls
/// `session.on_new_head_event` and publishes the resulting view model
/// onto the feed. This test drives `start_with_stream` with every port
/// stubbed, pushes a `NewHead`, and asserts the next view model carries
/// the re-primed "next head" fixture data.
#[tokio::test]
async fn start_with_stream_forwards_new_head_events_onto_the_feed() {
    let network = StubNetworkStatusPort::new();
    let gas = StubGasOraclePort::new();
    let new_heads = StubNewHeadsStreamPort::new();
    let chains = StubChainRegistry::with_all_enabled();

    network.set_snapshot(NetworkStatusFixture::load(
        "home__network_status__ethereum.json",
    ));
    gas.set_snapshot(GasSnapshotFixture::load(
        "home__gas_snapshot__ethereum.json",
    ));

    let session = HomeSession::new(network.clone(), gas.clone(), chains, Chain::Ethereum);
    let (mut feed, handle) = infra_home_feed::start_with_stream(
        session,
        new_heads.clone(),
        Chain::Ethereum,
        Duration::from_secs(600),
    );

    // Drain the initial view(s) so the next `recv()` is the one driven
    // by our WS push. The refresher sends on boot and again after
    // subscribe(), so we skip up to two initial snapshots.
    let initial = feed.recv().await.expect("initial snapshot");
    assert_eq!(
        initial.network.as_ref().unwrap().latest_block.value(),
        21_345_678
    );

    network.set_snapshot(NetworkStatusFixture::load(
        "home__network_status__ethereum_next_head.json",
    ));
    gas.set_snapshot(GasSnapshotFixture::load(
        "home__gas_snapshot__ethereum_next_head.json",
    ));

    new_heads.push_head(NewHead {
        chain: Chain::Ethereum,
        number: BlockNumber::new(21_345_679),
    });

    let view = loop {
        let next = feed.recv().await.expect("feed still open");
        if next.network.as_ref().unwrap().latest_block.value() == 21_345_679 {
            break next;
        }
    };
    assert_eq!(view.gas.as_ref().unwrap().average.value(), 17);

    // Tear down the refresher task.
    handle.abort();
}

/// plan/1-home.md §12.5: when the WS stream drops, the dispatcher flips
/// the session to Disconnected and keeps the polling timer alive as a
/// fallback.
#[tokio::test]
async fn start_with_stream_marks_disconnected_on_subscribe_failure() {
    let network = StubNetworkStatusPort::new();
    let gas = StubGasOraclePort::new();
    let new_heads = StubNewHeadsStreamPort::new();
    let chains = StubChainRegistry::with_all_enabled();

    network.set_snapshot(NetworkStatusFixture::load(
        "home__network_status__ethereum.json",
    ));
    gas.set_snapshot(GasSnapshotFixture::load(
        "home__gas_snapshot__ethereum.json",
    ));
    new_heads.set_broken(true);

    let session = HomeSession::new(network, gas, chains, Chain::Ethereum);
    let (mut feed, handle) = infra_home_feed::start_with_stream(
        session,
        new_heads,
        Chain::Ethereum,
        Duration::from_secs(600),
    );

    // The loop sends: (1) the initial refresh, (2) the drop-flag frame
    // after subscribe fails. Scan the feed until we see the
    // Disconnected state so the assertion is resilient to ordering.
    let disconnected = loop {
        let next = feed.recv().await.expect("feed still open");
        if matches!(
            next.connection,
            ConnectionStatus::Disconnected {
                reconnect_scheduled: true
            }
        ) {
            break next;
        }
    };

    // Cached snapshots must still be there even in the degraded state.
    assert!(disconnected.network.is_some());
    assert!(disconnected.gas.is_some());

    handle.abort();
}
