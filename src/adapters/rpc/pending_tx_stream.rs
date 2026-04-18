//! Alchemy WebSocket adapter for [`PendingTxStreamPort`].
//!
//! Connects to `wss://{alchemy_subdomain}.g.alchemy.com/v2/{api_key}`
//! and subscribes to `alchemy_pendingTransactions` (a superset of the
//! standard `newPendingTransactions` that emits the full tx object,
//! saving a hydration round-trip). Each incoming notification is
//! translated into a [`PendingTxEvent::Added`] and pushed into the
//! bounded channel handed back to the screen.
//!
//! See `plan/5-mempool.md` §11.3.4 for the full scope, including the
//! `newPendingTransactions` + `eth_getTransactionByHash` fallback
//! and the `newHeads` mined-removal synthesis that are still
//! deferred.

use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, Utf8Bytes},
};
use url::Url;

use crate::{
    application::ports::PendingTxStreamPort,
    domain::{
        Address, Chain, DomainError, PendingTx, PendingTxEvent, PendingTxFilter, TxHash, Wei,
    },
};

/// Alchemy-backed implementation of [`PendingTxStreamPort`]. Each
/// `subscribe` call spawns a task that owns one WebSocket connection
/// and forwards events until the stream dies or the screen drops the
/// receiver.
///
/// `update_filter` broadcasts the new predicate to the task running
/// the **latest** subscribe; this matches the single-subscriber UX
/// of the Mempool screen and avoids keeping dead tasks alive. When
/// called before any subscribe, `update_filter` fails with
/// [`DomainError::ProviderUnavailable`] — the composition root
/// always calls `subscribe` first so this path is unreachable in
/// practice.
#[derive(Debug, Clone)]
pub struct AlchemyPendingTxStream {
    ws_url: Url,
    latest_control: Arc<Mutex<Option<UnboundedSender<PendingTxFilter>>>>,
}

impl AlchemyPendingTxStream {
    /// Build an adapter bound to the given Alchemy WebSocket URL.
    /// The URL must already include the API key (the composition
    /// root owns secret handling).
    #[must_use]
    pub fn new(ws_url: Url) -> Self {
        Self {
            ws_url,
            latest_control: Arc::new(Mutex::new(None)),
        }
    }
}

impl PendingTxStreamPort for AlchemyPendingTxStream {
    async fn subscribe(
        &self,
        _chain: Chain,
        filter: PendingTxFilter,
    ) -> Result<UnboundedReceiver<PendingTxEvent>, DomainError> {
        let (events_tx, events_rx) = unbounded_channel::<PendingTxEvent>();
        let (control_tx, control_rx) = unbounded_channel::<PendingTxFilter>();

        // Replace the stored control sender so the next
        // `update_filter` reaches the freshly-spawned task. The
        // previous task (if any) will see its control channel drop
        // on the next recv and exit quietly.
        if let Ok(mut slot) = self.latest_control.lock() {
            *slot = Some(control_tx);
        }

        let ws_url = self.ws_url.clone();
        tokio::spawn(async move {
            // The task returns when the WS connection dies or the
            // receiver is dropped. Any error is swallowed because
            // closing the events channel is the signal the screen
            // uses to flip to Disconnected.
            let _ = run_subscription(ws_url, filter, events_tx, control_rx).await;
        });

        Ok(events_rx)
    }

    async fn update_filter(
        &self,
        _chain: Chain,
        filter: PendingTxFilter,
    ) -> Result<(), DomainError> {
        let sender = self
            .latest_control
            .lock()
            .ok()
            .and_then(|slot| slot.clone());
        let Some(sender) = sender else {
            return Err(DomainError::ProviderUnavailable);
        };
        sender
            .send(filter)
            .map_err(|_| DomainError::ProviderUnavailable)
    }
}

/// Single-connection subscription loop. Consumes both the WebSocket
/// stream and the in-process control channel; whichever fires first
/// drives the next iteration.
async fn run_subscription(
    ws_url: Url,
    initial_filter: PendingTxFilter,
    events_tx: UnboundedSender<PendingTxEvent>,
    mut control_rx: UnboundedReceiver<PendingTxFilter>,
) -> Result<(), DomainError> {
    let (mut stream, _response) = connect_async(ws_url.as_str())
        .await
        .map_err(|_| DomainError::ProviderUnavailable)?;

    let mut req_id: u64 = 1;
    let sub_request = build_subscribe_request(&initial_filter, req_id);
    stream
        .send(Message::Text(Utf8Bytes::from(sub_request.to_string())))
        .await
        .map_err(|_| DomainError::ProviderUnavailable)?;

    let mut current_sub_id: Option<String> = None;

    loop {
        tokio::select! {
            biased;
            maybe_filter = control_rx.recv() => {
                let Some(new_filter) = maybe_filter else {
                    // Adapter dropped: tear down the subscription.
                    return Ok(());
                };
                if let Some(id) = current_sub_id.take() {
                    req_id += 1;
                    let unsub = json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "method": "eth_unsubscribe",
                        "params": [id],
                    });
                    if stream
                        .send(Message::Text(Utf8Bytes::from(unsub.to_string())))
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }
                req_id += 1;
                let sub = build_subscribe_request(&new_filter, req_id);
                if stream
                    .send(Message::Text(Utf8Bytes::from(sub.to_string())))
                    .await
                    .is_err()
                {
                    return Ok(());
                }
            }
            maybe_msg = stream.next() => {
                let Some(msg) = maybe_msg else { return Ok(()); };
                let Ok(msg) = msg else { return Ok(()); };
                match msg {
                    Message::Text(text)
                        if handle_text(text.as_str(), &mut current_sub_id, &events_tx).is_break() =>
                    {
                        return Ok(());
                    }
                    Message::Text(_) => {}
                    Message::Close(_) => return Ok(()),
                    Message::Ping(payload) => {
                        let _ = stream.send(Message::Pong(payload)).await;
                    }
                    _ => {}
                }
            }
        }
    }
}

enum Flow {
    Continue,
    Break,
}

impl Flow {
    fn is_break(&self) -> bool {
        matches!(self, Flow::Break)
    }
}

fn handle_text(
    raw: &str,
    current_sub_id: &mut Option<String>,
    events_tx: &UnboundedSender<PendingTxEvent>,
) -> Flow {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        // Malformed frame: skip, keep the subscription alive.
        return Flow::Continue;
    };

    // Subscription confirmation: {"jsonrpc":"2.0","id":1,"result":"0xabc"}.
    if let Some(result) = value.get("result").and_then(Value::as_str) {
        *current_sub_id = Some(result.to_string());
        return Flow::Continue;
    }

    // Notification: {"method":"eth_subscription","params":{"subscription":"0xabc","result":{…}}}.
    if value.get("method").and_then(Value::as_str) == Some("eth_subscription")
        && let Some(tx_value) = value.pointer("/params/result")
        && let Some(pending) = parse_pending_tx(tx_value)
        && events_tx.send(PendingTxEvent::Added(pending)).is_err()
    {
        return Flow::Break;
    }

    Flow::Continue
}

fn build_subscribe_request(filter: &PendingTxFilter, req_id: u64) -> Value {
    let mut params = vec![json!("alchemy_pendingTransactions")];
    if let Some(from) = filter.from {
        // Alchemy filter syntax: second param is an object with
        // optional `fromAddress` / `toAddress` keys.
        params.push(json!({ "fromAddress": from.to_hex() }));
    }
    json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "method": "eth_subscribe",
        "params": params,
    })
}

fn parse_pending_tx(value: &Value) -> Option<PendingTx> {
    let hash_hex = value.get("hash")?.as_str()?;
    let from_hex = value.get("from")?.as_str()?;
    let to_hex = value.get("to").and_then(Value::as_str);
    let value_hex = value.get("value")?.as_str()?;

    let hash = TxHash::from_hex(hash_hex).ok()?;
    let from = Address::from_hex(from_hex).ok()?;
    let to = to_hex.and_then(|s| Address::from_hex(s).ok());
    let value = parse_hex_u128(value_hex)?;

    Some(PendingTx {
        hash,
        from,
        to,
        value: Wei::new(value),
    })
}

fn parse_hex_u128(s: &str) -> Option<u128> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    if stripped.is_empty() {
        return Some(0);
    }
    u128::from_str_radix(stripped, 16).ok()
}
