//! Background task that polls `GasOraclePort` and publishes
//! `GasSnapshot` updates into the Gas Tracker screen's feed.
//!
//! See `plan/9-gas-tracker.md` section 11.1.

use std::time::Duration;

use tokio::task::JoinHandle;

use crate::{adapters::ui::GasFeedSender, application::ports::GasOraclePort, domain::Chain};

pub const DEFAULT_REFRESH_PERIOD: Duration = Duration::from_secs(6);

pub fn spawn<O>(chain: Chain, oracle: O, sender: GasFeedSender, period: Duration) -> JoinHandle<()>
where
    O: GasOraclePort + 'static,
{
    tokio::spawn(async move {
        loop {
            if let Ok(snapshot) = oracle.snapshot(chain).await
                && sender.updates_tx.send(snapshot).is_err()
            {
                break;
            }
            tokio::time::sleep(period).await;
        }
    })
}
