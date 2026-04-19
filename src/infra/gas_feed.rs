//! Background task that polls `GasOraclePort` and publishes
//! `GasSnapshot` updates into the Gas Tracker screen's feed.
//!
//! See `plan/9-gas-tracker.md` §11.1 (MVP polling loop) and §11.2
//! (manual refresh kick via `Ctrl+R`).

use std::time::Duration;

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::{GasFeedSender, GasRefreshListener},
    application::ports::GasOraclePort,
    domain::Chain,
};

pub const DEFAULT_REFRESH_PERIOD: Duration = Duration::from_secs(6);

/// Spawn the polling loop with an optional `GasRefreshListener`. When
/// a kick arrives on the listener, the next `oracle.snapshot` call
/// happens immediately instead of waiting for the period timer. The
/// listener is drained of pending kicks so burst-pressing `Ctrl+R`
/// does not queue up multiple refetches.
pub fn spawn_with_refresh<O>(
    chain: Chain,
    oracle: O,
    sender: GasFeedSender,
    period: Duration,
    refresh: Option<GasRefreshListener>,
) -> JoinHandle<()>
where
    O: GasOraclePort + 'static,
{
    tokio::spawn(async move {
        let mut refresh = refresh;
        loop {
            if let Ok(snapshot) = oracle.snapshot(chain).await
                && sender.updates_tx.send(snapshot).is_err()
            {
                break;
            }
            if let Some(listener) = refresh.as_mut() {
                tokio::select! {
                    biased;
                    kick = listener.recv() => {
                        match kick {
                            Some(()) => {
                                // Drain extra kicks queued while we
                                // were awaiting the last snapshot so
                                // bursty Ctrl+R presses collapse to
                                // a single refetch per loop tick.
                                while listener.try_recv().is_some() {}
                            }
                            None => {
                                // Every handle was dropped — the
                                // screen is gone, exit the loop.
                                break;
                            }
                        }
                    }
                    () = tokio::time::sleep(period) => {}
                }
            } else {
                tokio::time::sleep(period).await;
            }
        }
    })
}
