//! Outbound port that returns the slow/average/fast gas tiers plus base
//! fee history for a chain.
//!
//! See `plan/1-home.md` section 4.2 and `plan/9-gas-tracker.md`.

use crate::domain::{Chain, DomainError, GasSnapshot};

pub trait GasOraclePort: Send + Sync {
    /// Return the current gas snapshot for the given chain. The snapshot
    /// contains slow/average/fast tiers in gwei, the current base fee and
    /// the recent base-fee trend.
    fn snapshot(
        &self,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<GasSnapshot, DomainError>> + Send;
}
