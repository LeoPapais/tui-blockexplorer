//! Outbound port that feeds the Home dispatcher with `newHeads` events.
//!
//! See `plan/1-home.md` section 12.2. Live adapter is the Alchemy
//! WebSocket subscription `eth_subscribe("newHeads")`; the composition
//! root may also wire a `Disabled` degradation so polling remains the
//! live behaviour until the real adapter ships.

use tokio::sync::mpsc::UnboundedReceiver;

use crate::domain::{Chain, DomainError, NewHead};

pub trait NewHeadsStreamPort: Send + Sync {
    /// Open a `newHeads` subscription on the given chain. The receiver
    /// produces one [`NewHead`] per block until dropped; closing the
    /// sender side signals the upstream connection went away (the
    /// dispatcher then marks the session disconnected and keeps
    /// polling as a fallback).
    fn subscribe(
        &self,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<UnboundedReceiver<NewHead>, DomainError>> + Send;
}
