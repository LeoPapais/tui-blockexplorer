//! Polling [`TokenPriceStreamPort`] adapter that composes an existing
//! [`PricesPort`] and re-issues `get_single` on a fixed cadence.
//!
//! The Alchemy Prices API does not expose a push / WebSocket surface
//! for spot-price updates as of MVP, so we fake the stream by polling
//! every `interval` (15 s by default) and dropping each sample onto an
//! unbounded channel. This keeps the Token Detail dispatcher honest —
//! the UI consumes an `UnboundedReceiver<PriceLookup>` the same way it
//! would consume a real push stream, and swapping the adapter later
//! does not require UI changes.
//!
//! Transport failures from the underlying port are swallowed: the
//! stream stays silent for that tick rather than closing. A real
//! reconnect / backoff policy can be layered on top if the provider
//! becomes flaky; for now we match the "best-effort" semantics used by
//! `src/infra/home_feed.rs` for the `newHeads` WebSocket.
//!
//! See `plan/8-token-detail.md` §13.1 and `plan/15-backlog.md` §8.9.

use std::time::Duration;

use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use crate::{
    application::ports::{PricesPort, TokenPriceStreamPort},
    domain::{Address, Chain, DomainError, PriceLookup},
};

/// Default polling interval. Conservative enough to stay well below
/// Alchemy's CU budget while still feeling live on the Chart tab.
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(15);

/// Adapter that turns any [`PricesPort`] into a
/// [`TokenPriceStreamPort`] by polling `get_single` on a fixed
/// interval.
///
/// `interval` must be strictly positive; a zero duration would make
/// the polling loop starve the scheduler.
#[derive(Clone, Debug)]
pub struct PollingTokenPriceStream<P> {
    prices: P,
    interval: Duration,
}

impl<P> PollingTokenPriceStream<P> {
    /// Build a polling stream with an explicit interval. Callers that
    /// want the documented 15-second default should use
    /// [`PollingTokenPriceStream::with_default_interval`].
    #[must_use]
    pub fn new(prices: P, interval: Duration) -> Self {
        assert!(
            !interval.is_zero(),
            "polling interval must be strictly positive"
        );
        Self { prices, interval }
    }

    /// Shorthand for [`PollingTokenPriceStream::new`] with the
    /// [`DEFAULT_POLL_INTERVAL`] cadence.
    #[must_use]
    pub fn with_default_interval(prices: P) -> Self {
        Self::new(prices, DEFAULT_POLL_INTERVAL)
    }
}

impl<P> TokenPriceStreamPort for PollingTokenPriceStream<P>
where
    P: PricesPort + Clone + 'static,
{
    async fn subscribe(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<UnboundedReceiver<PriceLookup>, DomainError> {
        let (tx, rx) = unbounded_channel();
        let prices = self.prices.clone();
        let interval = self.interval;
        tokio::spawn(async move {
            // Warm-up: emit the first sample immediately so the UI
            // can flip out of the `Pending` state without waiting a
            // full interval.
            if let Ok(lookup) = prices.get_single(address, chain).await
                && tx.send(lookup).is_err()
            {
                return;
            }
            loop {
                tokio::time::sleep(interval).await;
                match prices.get_single(address, chain).await {
                    Ok(lookup) => {
                        if tx.send(lookup).is_err() {
                            break;
                        }
                    }
                    // Best-effort: keep the stream open on transient
                    // errors. If the receiver is gone, the next
                    // successful send will fail and we break out.
                    Err(_) => continue,
                }
            }
        });
        Ok(rx)
    }
}
