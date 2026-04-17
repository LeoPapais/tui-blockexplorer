//! Outbound port that surfaces the current state of the active chain
//! (latest block, base fee, block-time average).
//!
//! See `plan/1-home.md` section 4.1.

use crate::domain::{Chain, DomainError, NetworkStatus};

pub trait NetworkStatusPort: Send + Sync {
    /// Return a fresh snapshot of the chain's head state.
    fn snapshot(
        &self,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<NetworkStatus, DomainError>> + Send;
}
