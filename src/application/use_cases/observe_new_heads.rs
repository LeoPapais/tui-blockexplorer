//! Atomic use case: open a `newHeads` subscription via the
//! [`NewHeadsStreamPort`] and hand the resulting receiver back to the
//! caller.
//!
//! The dispatcher is the typical consumer: it `select!`s between the
//! returned channel and the existing 6-second polling timer so the
//! Home screen stays live while the WS connection is healthy and
//! still refreshes when it is not.
//!
//! See `plan/1-home.md` section 12.3.

use tokio::sync::mpsc::UnboundedReceiver;

use crate::{
    application::ports::NewHeadsStreamPort,
    domain::{Chain, DomainError, NewHead},
};

/// Open a `newHeads` subscription against the provided port.
pub async fn run<P: NewHeadsStreamPort>(
    port: &P,
    chain: Chain,
) -> Result<UnboundedReceiver<NewHead>, DomainError> {
    port.subscribe(chain).await
}
