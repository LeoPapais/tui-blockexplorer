//! Background refresher that drives a [`HomeSession`] and pushes the
//! resulting view models into a [`HomeFeed`].
//!
//! See `plan/14-config-and-credentials.md` section 3.2 for the polling
//! path and `plan/1-home.md` section 12.5 for the WebSocket path.

use std::time::Duration;

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::{HomeFeedSender, home_feed},
    application::{
        HomeSession,
        ports::{ChainRegistryPort, GasOraclePort, NetworkStatusPort, NewHeadsStreamPort},
    },
    domain::Chain,
};

/// Default interval between refreshes. Matches the plan's "every N
/// seconds" guidance; 6 seconds is slow enough not to hammer the API
/// and fast enough to feel live on chains with ~12s block times.
pub const DEFAULT_REFRESH_PERIOD: Duration = Duration::from_secs(6);

/// Start a polling-only refresher task. Returns the receiving half of
/// the feed and a join handle pointing at the spawned task.
///
/// This is the MVP entry point used while the real Alchemy WebSocket
/// adapter is still deferred (see `plan/1-home.md` §12.4). Once the
/// adapter ships, wire it through [`start_with_stream`] instead.
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

/// Start a refresher task that composes a `newHeads` subscription with
/// the polling timer. See `plan/1-home.md` §12.5 for the exact
/// select-loop contract:
///
/// - WS event → `session.on_new_head_event(head)`.
/// - WS stream drops → `session.on_connection_drop()` and keep the
///   timer running; polling remains a belt-and-suspenders source of
///   refreshes until the stream is re-established on the next
///   iteration.
/// - Timer tick → `session.refresh()` as today.
pub fn start_with_stream<N, G, C, S>(
    mut session: HomeSession<N, G, C>,
    stream: S,
    chain: Chain,
    period: Duration,
) -> (crate::adapters::ui::HomeFeed, JoinHandle<()>)
where
    N: NetworkStatusPort + 'static,
    G: GasOraclePort + 'static,
    C: ChainRegistryPort + 'static,
    S: NewHeadsStreamPort + 'static,
{
    let (feed, sender) = home_feed();
    let handle = tokio::spawn(async move {
        streamed_refresher_loop(&mut session, &stream, chain, sender, period).await;
    });
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

async fn streamed_refresher_loop<N, G, C, S>(
    session: &mut HomeSession<N, G, C>,
    stream: &S,
    chain: Chain,
    sender: HomeFeedSender,
    period: Duration,
) where
    N: NetworkStatusPort,
    G: GasOraclePort,
    C: ChainRegistryPort,
    S: NewHeadsStreamPort,
{
    // Initial population so the screen is never stuck on "Loading..."
    // while we wait for the first head.
    let _ = session.refresh().await;
    if sender.send(session.view().clone()).is_err() {
        return;
    }

    let mut rx = match stream.subscribe(chain).await {
        Ok(rx) => Some(rx),
        Err(_) => {
            session.on_connection_drop();
            let _ = sender.send(session.view().clone());
            None
        }
    };

    loop {
        let timer = tokio::time::sleep(period);
        tokio::pin!(timer);

        tokio::select! {
            biased;
            maybe_head = async {
                match rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                match maybe_head {
                    Some(head) => {
                        let _ = session.on_new_head_event(head).await;
                    }
                    None => {
                        // Upstream WS dropped: flip to Disconnected so
                        // the header surfaces the reconnect hint and
                        // rely on the timer to keep polling.
                        session.on_connection_drop();
                        rx = None;
                    }
                }
            }
            _ = &mut timer => {
                let _ = session.refresh().await;

                // Opportunistically re-subscribe if the WS stream had
                // dropped: a successful subscribe implicitly flips the
                // session back to Connected on the next refresh.
                if rx.is_none()
                    && let Ok(new_rx) = stream.subscribe(chain).await
                {
                    rx = Some(new_rx);
                }
            }
        }

        if sender.send(session.view().clone()).is_err() {
            break;
        }
    }
}
