//! State-diff trace output used by the TxDetail `State Changes` tab
//! and the call-tree used by the `Internal` tab.
//!
//! `StateDiff` mirrors Parity's `trace_replayTransaction` `stateDiff`
//! shape; `CallNode` mirrors the `callTracer` output. Both are thin
//! domain wrappers so the UI stays provider-agnostic.
//!
//! See `plan/4-tx-detail.md` sections 12.4.3 and 12.6.5.

use crate::domain::{Address, Wei};

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

/// Which EVM call primitive produced a frame in the call tree.
/// Mirrors the variants `callTracer` emits plus the Parity-style
/// selfdestruct / create2 cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    Call,
    Callcode,
    Delegatecall,
    Staticcall,
    Create,
    Create2,
    Selfdestruct,
    /// Any provider-specific kind the adapter did not recognise.
    /// The UI falls back to rendering the raw string.
    Unknown,
}

impl CallKind {
    /// Three-letter-ish label used by the Internal tab. Kept short
    /// so the tree stays readable at 80 columns.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            CallKind::Call => "CALL",
            CallKind::Callcode => "CALLCODE",
            CallKind::Delegatecall => "DELEGATECALL",
            CallKind::Staticcall => "STATICCALL",
            CallKind::Create => "CREATE",
            CallKind::Create2 => "CREATE2",
            CallKind::Selfdestruct => "SELFDESTRUCT",
            CallKind::Unknown => "CALL?",
        }
    }

    /// Parse the string used by Geth's `callTracer` (e.g. `"CALL"`,
    /// `"DELEGATECALL"`). Case-insensitive; unknown strings map to
    /// [`CallKind::Unknown`].
    #[must_use]
    pub fn from_str_upper(s: &str) -> Self {
        match s.to_ascii_uppercase().as_str() {
            "CALL" => CallKind::Call,
            "CALLCODE" => CallKind::Callcode,
            "DELEGATECALL" => CallKind::Delegatecall,
            "STATICCALL" => CallKind::Staticcall,
            "CREATE" => CallKind::Create,
            "CREATE2" => CallKind::Create2,
            "SUICIDE" | "SELFDESTRUCT" => CallKind::Selfdestruct,
            _ => CallKind::Unknown,
        }
    }
}

/// One frame of the transaction call tree.
///
/// Produced by `TxTracePort::call_tree` and consumed by the Internal
/// tab on `TxDetailScreen`. The tree root is the top-level call
/// initiated by the transaction sender; `children` holds
/// sub-calls in execution order.
///
/// `to` is `None` only for `CREATE` / `CREATE2` frames whose
/// resulting address is unavailable in the tracer output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallNode {
    pub kind: CallKind,
    pub from: Address,
    pub to: Option<Address>,
    pub value: Wei,
    pub input: Vec<u8>,
    pub output: Vec<u8>,
    pub gas_used: u64,
    /// Reverted / out-of-gas / precompile error message captured
    /// verbatim from the tracer. The UI tags the line in red when
    /// this is `Some`.
    pub error: Option<String>,
    pub children: Vec<CallNode>,
}

impl CallNode {
    /// Total number of frames in the subtree (including `self`).
    #[must_use]
    pub fn frame_count(&self) -> usize {
        1 + self
            .children
            .iter()
            .map(CallNode::frame_count)
            .sum::<usize>()
    }
}
