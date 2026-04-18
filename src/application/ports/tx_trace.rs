//! Outbound port returning trace data for a mined transaction:
//! Parity-style state-diff for the `State Changes` tab, and a
//! tree-shaped call tree for the `Internal` tab.
//!
//! Backed by `trace_replayTransaction` with `["stateDiff"]`, and by
//! `trace_transaction` / `debug_traceTransaction` (`callTracer`)
//! for the tree. Not every chain / provider tier supports these;
//! callers must handle `DomainError::FeatureUnavailable` gracefully
//! (the UI falls back to an informational message).
//!
//! See `plan/4-tx-detail.md` sections 12.4.3 and 12.6.5.

use crate::domain::{CallNode, Chain, DomainError, StateDiff, TxHash};

pub trait TxTracePort: Send + Sync {
    fn state_diff(
        &self,
        hash: TxHash,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<StateDiff, DomainError>> + Send;

    /// Fetch the full call tree for `hash`. Returns
    /// `DomainError::FeatureUnavailable` when neither the
    /// `trace_transaction` Parity namespace nor
    /// `debug_traceTransaction` with `callTracer` are supported by
    /// the provider tier.
    ///
    /// Default implementation returns `FeatureUnavailable` so that
    /// adapters / stubs can opt in without forcing every existing
    /// caller to reach for a tree.
    fn call_tree(
        &self,
        hash: TxHash,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<CallNode, DomainError>> + Send {
        let _ = (hash, chain);
        async { Err(DomainError::FeatureUnavailable) }
    }
}
