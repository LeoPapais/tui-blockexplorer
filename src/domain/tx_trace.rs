//! State-diff trace output used by the TxDetail `State Changes` tab.
//!
//! Mirrors Parity's `trace_replayTransaction` stateDiff shape with a
//! thin domain wrapper so the UI stays provider-agnostic.
//!
//! See `plan/4-tx-detail.md` section 12.4.3.

use crate::domain::Address;

/// Transition shape shared by balance, nonce, code and storage
/// slots: they may be unchanged, added (the slot was empty before),
/// changed (known old value -> new value) or deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffChange {
    Unchanged,
    Added(String),
    Removed(String),
    Changed { from: String, to: String },
}

impl DiffChange {
    #[must_use]
    pub fn is_change(&self) -> bool {
        !matches!(self, DiffChange::Unchanged)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageSlotDiff {
    /// Slot key (hex including leading `0x`).
    pub slot: String,
    pub change: DiffChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressStateDiff {
    pub address: Address,
    pub balance: DiffChange,
    pub nonce: DiffChange,
    pub code: DiffChange,
    pub storage: Vec<StorageSlotDiff>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StateDiff {
    pub entries: Vec<AddressStateDiff>,
}

impl StateDiff {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
