//! Outbound port returning simulated asset-change deltas for a tx.
//!
//! Backed by Alchemy's `alchemy_simulateAssetChanges` API in live
//! mode. See `plan/4-tx-detail.md` section 12.4.3.

use crate::domain::{AssetChange, Chain, DomainError, Transaction};

pub trait TxSimulationPort: Send + Sync {
    fn simulate_asset_changes(
        &self,
        tx: &Transaction,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Vec<AssetChange>, DomainError>> + Send;
}
