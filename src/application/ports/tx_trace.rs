//! Outbound port returning a state-diff for a mined transaction.
//!
//! Backed by `trace_replayTransaction` with `["stateDiff"]`. Not
//! every chain / provider tier supports this; callers must handle
//! `DomainError::ProviderUnavailable` gracefully (the UI falls back
//! to an informational message).
//!
//! See `plan/4-tx-detail.md` section 12.4.3.

use crate::domain::{Chain, DomainError, StateDiff, TxHash};

pub trait TxTracePort: Send + Sync {
    fn state_diff(
        &self,
        hash: TxHash,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<StateDiff, DomainError>> + Send;
}
