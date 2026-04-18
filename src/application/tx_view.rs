//! View-model consumed by the Tx Detail screen.
//!
//! Wraps the raw [`Transaction`] entity with enrichment produced by
//! downstream use cases: decoded method, decoded logs, and (in
//! commit 3 of plan/4) asset / state changes.
//!
//! See `plan/4-tx-detail.md` section 12.4.2.

use crate::domain::{LogEntry, Transaction};

/// Where a decoded signature came from. Surfaced to the UI so users
/// can tell an ABI-backed decoding from a best-effort 4byte lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureSource {
    /// Signature extracted from the contract's verified ABI.
    Abi,
    /// Signature pulled from a 4byte-compatible directory.
    SignatureDirectory,
}

impl SignatureSource {
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            SignatureSource::Abi => "from ABI",
            SignatureSource::SignatureDirectory => "from sigdb",
        }
    }
}

/// Decoded method for the Overview `Method` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedMethod {
    /// Canonical signature text, e.g. `transfer(address,uint256)`.
    pub signature: String,
    pub source: SignatureSource,
}

/// Decoded receipt log. Holds the original raw entry so the screen
/// can still render topics + data when no signature matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedLog {
    pub raw: LogEntry,
    pub signature: Option<DecodedSignature>,
}

/// Similar to `DecodedMethod` but carries the decoded event signature
/// for a receipt log. Kept as a separate type for discoverability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedSignature {
    pub signature: String,
    pub source: SignatureSource,
}

/// Enriched view consumed by `TxDetailScreen`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxView {
    pub tx: Transaction,
    /// `None` when the tx has no calldata (plain ETH transfer) or no
    /// signature match was found. The screen falls back to the raw
    /// 4-byte selector in that case.
    pub decoded_method: Option<DecodedMethod>,
    pub decoded_logs: Vec<DecodedLog>,
}

impl TxView {
    #[must_use]
    pub fn bare(tx: Transaction) -> Self {
        let decoded_logs = tx
            .logs
            .iter()
            .map(|raw| DecodedLog {
                raw: raw.clone(),
                signature: None,
            })
            .collect();
        Self {
            tx,
            decoded_method: None,
            decoded_logs,
        }
    }
}
