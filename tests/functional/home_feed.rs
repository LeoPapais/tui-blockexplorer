//! Functional tests for the `HomeScreen` feed channel.
//!
//! See `plan/14-config-and-credentials.md` section 3.

use blockexplorer_tui::{
    adapters::ui::{HomeScreen, Screen, home_feed},
    application::{ConnectionStatus, HomeViewModel},
    domain::{BlockNumber, Chain, GasSnapshot, Gwei, NetworkStatus, Wei},
};
use pretty_assertions::assert_eq;

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
