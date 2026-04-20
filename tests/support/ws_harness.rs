//! In-process WebSocket test harness for the Alchemy pending-tx
//! adapter.
//!
//! Lives here because `wiremock` cannot stand in for a WebSocket
//! server. Uses `tokio::net::TcpListener` + `tokio_tungstenite::
//! accept_async` to perform a real WS handshake; the server script
//! is driven by a list of `Step`s that either wait for the adapter
//! to send a specific JSON-RPC method or push a canned frame.
//!
//! See `plan/13-alchemy-adapter.md` (WebSocket testing).

use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::{
    net::TcpListener,
    sync::mpsc::{UnboundedSender, unbounded_channel},
    task::JoinHandle,
};
use tokio_tungstenite::tungstenite::{Message, Utf8Bytes};
use url::Url;

/// A scripted interaction with the connecting WebSocket client.
#[derive(Debug, Clone)]
pub enum WsStep {
    /// Wait for a JSON-RPC `eth_subscribe` / `eth_unsubscribe` call
    /// whose `method` matches `expected_method`, capture the `id`,
    /// and reply with `{"jsonrpc":"2.0","id":<captured>,"result":<sub_id>}`.
    ExpectSubscribeReply {
        expected_method: &'static str,
        sub_id: &'static str,
    },
    /// Push a canned `eth_subscription` notification carrying the
    /// given `params.result` payload.
    EmitNotification { sub_id: &'static str, result: Value },
}

/// Running canned WebSocket server. Drop the value to shut it down.
pub struct CannedWsServer {
    url: Url,
    handle: JoinHandle<()>,
    capture_tx: UnboundedSender<CapturedMessage>,
}

/// Messages captured from the client. Not yet drained by any test —
/// reserved for a future enhancement that pins the exact
/// `eth_subscribe` / `eth_unsubscribe` frames the adapter sent.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum CapturedMessage {
    Text(String),
}

impl CannedWsServer {
    /// Start a server that executes the given script on the first
    /// incoming WebSocket connection. The returned [`Url`] is a
    /// valid `ws://127.0.0.1:{port}` the adapter can connect to
    /// with `tokio_tungstenite::connect_async`.
    pub async fn start(script: Vec<WsStep>) -> Self {
        let (capture_tx, _capture_rx) = unbounded_channel::<CapturedMessage>();
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .expect("bind localhost:0");
        let addr = listener.local_addr().expect("local_addr");
        let url = Url::parse(&format!("ws://{}", addr)).expect("valid ws url");

        let handle = tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await else {
                return;
            };

            for step in script {
                match step {
                    WsStep::ExpectSubscribeReply {
                        expected_method,
                        sub_id,
                    } => {
                        let captured_id: u64 = loop {
                            let Some(msg) = ws.next().await else {
                                return;
                            };
                            let Ok(Message::Text(text)) = msg else {
                                continue;
                            };
                            let Ok(value) = serde_json::from_str::<Value>(text.as_str()) else {
                                continue;
                            };
                            let method = value.get("method").and_then(Value::as_str);
                            if method == Some(expected_method) {
                                let id = value.get("id").and_then(Value::as_u64).unwrap_or(0);
                                break id;
                            }
                        };
                        let reply = json!({
                            "jsonrpc": "2.0",
                            "id": captured_id,
                            "result": sub_id,
                        });
                        if ws
                            .send(Message::Text(Utf8Bytes::from(reply.to_string())))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    WsStep::EmitNotification { sub_id, result } => {
                        let notification = json!({
                            "jsonrpc": "2.0",
                            "method": "eth_subscription",
                            "params": {
                                "subscription": sub_id,
                                "result": result,
                            },
                        });
                        if ws
                            .send(Message::Text(Utf8Bytes::from(notification.to_string())))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                }
            }
            // Keep the connection alive so the adapter task does not
            // race us to a close frame. The caller drops the handle
            // when the scenario is done.
            while let Some(msg) = ws.next().await {
                if msg.is_err() {
                    break;
                }
            }
        });

        Self {
            url,
            handle,
            capture_tx,
        }
    }

    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }
}

impl Drop for CannedWsServer {
    fn drop(&mut self) {
        // Make sure we do not leak the accept task when a test panics
        // before its own `.await` completes.
        self.handle.abort();
        // Keep the capture_tx field named so clippy does not complain
        // about an unused-field; a future assertion helper may drain
        // the receiver to pin the exact frames the adapter sent.
        drop(self.capture_tx.clone());
    }
}
