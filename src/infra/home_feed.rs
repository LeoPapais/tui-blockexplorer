//! Background refresher that drives a [`HomeSession`] and pushes the
//! resulting view models into a [`HomeFeed`].
//!
//! See `plan/14-config-and-credentials.md` section 3.2.

use std::time::Duration;

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::{HomeFeedSender, home_feed},
    application::{
        HomeSession,
        ports::{ChainRegistryPort, GasOraclePort, NetworkStatusPort},
    },
};

/// Default interval between refreshes. Matches the plan's "every N
/// seconds" guidance; 6 seconds is slow enough not to hammer the API
/// and fast enough to feel live on chains with ~12s block times.
pub const DEFAULT_REFRESH_PERIOD: Duration = Duration::from_secs(6);

/// Start a refresher task. Returns the receiving half of the feed and
/// a join handle pointing at the spawned task.
pub fn start<N, G, C>(
    mut session: HomeSession<N, G, C>,
    period: Duration,
) -> (crate::adapters::ui::HomeFeed, JoinHandle<()>)
where
    N: NetworkStatusPort + 'static,
    G: GasOraclePort + 'static,
    C: ChainRegistryPort + 'static,
{
    let (feed, sender) = home_feed();
    let handle = tokio::spawn(async move { refresher_loop(&mut session, sender, period).await });
    (feed, handle)
}

async fn refresher_loop<N, G, C>(
    session: &mut HomeSession<N, G, C>,
    sender: HomeFeedSender,
    period: Duration,
) where
    N: NetworkStatusPort,
    G: GasOraclePort,
    C: ChainRegistryPort,
{
    loop {
        // Best-effort: ignore the Result. `HomeSession::refresh` already
        // translates provider failures into the view model's connection
        // status, so we always have something publishable.
        let _ = session.refresh().await;

        if sender.send(session.view().clone()).is_err() {
            // Receiver dropped: the screen is gone; stop the loop.
            break;
        }

        tokio::time::sleep(period).await;
    }
}
