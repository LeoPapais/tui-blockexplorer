//! Atomic use case: pull the current [`NetworkStatus`] from the
//! [`NetworkStatusPort`].
//!
//! See `plan/1-home.md` section 4.1.

use crate::{
    application::ports::NetworkStatusPort,
    domain::{Chain, DomainError, NetworkStatus},
};

/// Execute the use case against the provided port.
pub async fn run<P: NetworkStatusPort>(
    port: &P,
    chain: Chain,
) -> Result<NetworkStatus, DomainError> {
    port.snapshot(chain).await
}
