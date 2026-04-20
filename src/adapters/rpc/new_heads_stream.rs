//! Alchemy WebSocket adapter for [`NewHeadsStreamPort`].
//!
//! Subscribes to the standard `eth_subscribe(["newHeads"])` stream over
//! a WebSocket connection.
//! Reconnects on transient drops using the jittered backoff schedule
//! from [`super::retry::RetryPolicy`] so the reconnect cadence
//! matches the HTTP retry story.
//!
//! See `plan/13-alchemy-adapter.md` §8.6 and
//! `plan/15-backlog.md` §8.14 item 1.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, Utf8Bytes},
};
use url::Url;

use super::retry::RetryPolicy;
use crate::{
    application::ports::{NewHeadsStreamPort, Rng},
    domain::{BlockNumber, Chain, DomainError, NewHead},
};

/// Alchemy-backed implementation of [`NewHeadsStreamPort`]. Each
/// `subscribe` call spawns a task that owns one WebSocket connection
/// and forwards `NewHead` events until the stream dies or the
/// listener drops the receiver.
///
/// The adapter owns a [`RetryPolicy`] used exclusively for reconnect
/// scheduling; once connected, every notification is forwarded
/// synchronously (no retry on forward).
#[derive(Debug, Clone)]
pub struct AlchemyNewHeadsStream {
    ws_url: Url,
    retry: Arc<RetryPolicy>,
}

impl AlchemyNewHeadsStream {
    /// Build an adapter bound to the given Alchemy WebSocket URL.
    /// The URL must already include the API key (the composition
    /// root owns secret handling).
    ///
    /// The reconnect schedule defaults to
    /// [`RetryPolicy::default_with_rng`].
    #[must_use]
    pub fn new(ws_url: Url, rng: Arc<dyn Rng>) -> Self {
        Self {
            ws_url,
            retry: Arc::new(RetryPolicy::default_with_rng(rng)),
        }
    }

    /// Construct with an explicit [`RetryPolicy`] — useful for tests
    /// that want very short reconnect delays and tight attempt
    /// caps.
    #[must_use]
    pub fn with_retry_policy(ws_url: Url, retry: RetryPolicy) -> Self {
        Self {
            ws_url,
            retry: Arc::new(retry),
        }
    }
}

impl NewHeadsStreamPort for AlchemyNewHeadsStream {
    async fn subscribe(&self, chain: Chain) -> Result<UnboundedReceiver<NewHead>, DomainError> {
        let (events_tx, events_rx) = unbounded_channel::<NewHead>();
        let ws_url = self.ws_url.clone();
        let retry = self.retry.clone();
        tokio::spawn(async move {
            // Swallow any error: dropping the events sender is the
            // signal the Home dispatcher uses to flip to
            // "disconnected" and resume polling.
            let _ = run_with_reconnect(ws_url, chain, retry, events_tx).await;
        });
        Ok(events_rx)
    }
}

async fn run_with_reconnect(
    ws_url: Url,
    chain: Chain,
    retry: Arc<RetryPolicy>,
    events_tx: UnboundedSender<NewHead>,
) -> Result<(), DomainError> {
    let max_attempts = retry.max_attempts();
    let mut failed_attempts = 0usize;
    loop {
        match run_single_connection(&ws_url, chain, &events_tx).await {
            Ok(()) => {
                // Remote closed cleanly or the consumer dropped the
                // receiver: stop the loop.
                return Ok(());
            }
            Err(_err) => {
                failed_attempts += 1;
                if failed_attempts >= max_attempts {
                    return Err(DomainError::ProviderUnavailable);
                }
                let delay = retry.backoff_for(failed_attempts - 1);
                tokio::time::sleep(delay).await;
            }
        }
    }
}

async fn run_single_connection(
    ws_url: &Url,
    chain: Chain,
    events_tx: &UnboundedSender<NewHead>,
) -> Result<(), ()> {
    let (mut stream, _response) = connect_async(ws_url.as_str()).await.map_err(|_| ())?;

    let sub_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_subscribe",
        "params": ["newHeads"],
    });
    stream
        .send(Message::Text(Utf8Bytes::from(sub_request.to_string())))
        .await
        .map_err(|_| ())?;

    while let Some(msg) = stream.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Some(head) = parse_new_head(text.as_str(), chain)
                    && events_tx.send(head).is_err()
                {
                    // Receiver dropped: exit cleanly so the
                    // reconnect loop stops.
                    return Ok(());
                }
            }
            Ok(Message::Close(_)) => return Ok(()),
            Ok(Message::Ping(payload)) => {
                if stream.send(Message::Pong(payload)).await.is_err() {
                    return Err(());
                }
            }
            Ok(_) => {}
            Err(_) => return Err(()),
        }
    }
    // Stream closed without an explicit close frame: treat as
    // transient so the reconnect loop schedules another attempt.
    Err(())
}

fn parse_new_head(raw: &str, chain: Chain) -> Option<NewHead> {
    let value: Value = serde_json::from_str(raw).ok()?;
    if value.get("method").and_then(Value::as_str) != Some("eth_subscription") {
        return None;
    }
    let number_hex = value.pointer("/params/result/number")?.as_str()?;
    let stripped = number_hex.strip_prefix("0x").unwrap_or(number_hex);
    let raw_number = u64::from_str_radix(stripped, 16).ok()?;
    Some(NewHead {
        chain,
        number: BlockNumber::new(raw_number),
    })
}
