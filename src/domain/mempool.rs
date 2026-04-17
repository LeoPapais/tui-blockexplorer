//! Domain types for the Mempool screen.
//!
//! See `plan/5-mempool.md` section 11.1.

use crate::domain::{Address, TxHash, Wei};

/// A single pending transaction as observed by the configured stream
/// port. Intentionally lean: the Mempool screen needs just enough to
/// render a row and drill into TxDetail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTx {
    pub hash: TxHash,
    pub from: Address,
    /// `None` marks a pending contract creation.
    pub to: Option<Address>,
    pub value: Wei,
}

/// Delta emitted by the stream. `Added` is produced when Alchemy
/// announces a new pending tx; `Removed` is synthesised either from a
/// `newHeads` hit (tx got mined) or from a `replaced` notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingTxEvent {
    Added(PendingTx),
    Removed(TxHash),
}

/// Client-side filter applied by the Mempool screen to the stream.
/// MVP only supports `from`; other axes arrive with the filter modal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PendingTxFilter {
    pub from: Option<Address>,
}

impl PendingTxFilter {
    /// True when `tx` satisfies every set predicate.
    #[must_use]
    pub fn matches(&self, tx: &PendingTx) -> bool {
        if let Some(from) = self.from
            && tx.from != from
        {
            return false;
        }
        true
    }
}
