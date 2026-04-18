//! View-model consumed by the Tx Detail screen.
//!
//! Wraps the raw [`Transaction`] entity with enrichment produced by
//! downstream use cases: decoded method, decoded logs, and (in
//! commit 3 of plan/4) asset / state changes.
//!
//! See `plan/4-tx-detail.md` section 12.4.2.

use crate::domain::{Address, AssetChange, CallNode, LogEntry, StateDiff, Transaction};

/// Where a decoded signature came from. Surfaced to the UI so users
/// can tell an ABI-backed decoding from a best-effort directory
/// lookup, and — within the directory fallback chain — which mirror
/// resolved the selector.
///
/// See `.cursor/rules/external-apis.mdc` (fallback chain: ABI →
/// openchain → Samczsun → raw) and `plan/15-backlog.md` section 3.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureSource {
    /// Signature extracted from the contract's verified ABI.
    Abi,
    /// Signature extracted from the verified ABI of a proxy's
    /// implementation, reached by following the EIP-1967 slot on
    /// `proxy`. See `plan/15-backlog.md` section 3.3.
    ProxyAbi {
        proxy: Address,
        implementation: Address,
    },
    /// Signature pulled from `api.openchain.xyz`.
    Openchain,
    /// Signature pulled from the Samczsun signature DB mirror.
    Samczsun,
}

impl SignatureSource {
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            SignatureSource::Abi => "from ABI",
            SignatureSource::ProxyAbi { .. } => "from proxy→implementation",
            SignatureSource::Openchain => "from openchain",
            SignatureSource::Samczsun => "from samczsun",
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
    /// Populated when the signature was resolved through an ABI
    /// entry (direct or proxy implementation). The UI uses it to
    /// align `topics[1..]` and `data` words onto the real
    /// indexed / non-indexed split, instead of assuming the first
    /// N positional args are the indexed ones (which only happens
    /// to match canonical ERC20/ERC721 events).
    ///
    /// Signature-directory hits (openchain / Samczsun) leave this
    /// `None`: the 4byte mirror cannot tell indexed apart from
    /// non-indexed.
    ///
    /// See `plan/4-tx-detail.md` section 12.6.4.
    pub parsed: Option<EventAbi>,
}

/// Parsed event description pulled out of an ABI entry. Used by
/// the Logs tab to honour the real indexed / non-indexed split.
/// See [`DecodedSignature::parsed`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventAbi {
    pub name: String,
    pub params: Vec<EventParamAbi>,
}

/// One positional parameter of an ABI event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventParamAbi {
    /// Canonical argument name. May be empty for anonymous
    /// parameters; the renderer falls back to `arg{index}` in that
    /// case.
    pub name: String,
    /// Canonical Solidity type string (e.g. `address`,
    /// `uint256`, `bytes32`). Preserved verbatim from the ABI.
    pub type_: String,
    /// Whether the parameter is marked `indexed` in the ABI.
    /// Indexed parameters go into `topics[1..]`, non-indexed into
    /// `data`.
    pub indexed: bool,
}

/// Status of a deferred / feature-gated enrichment. The UI uses
/// this to tell "not yet loaded" apart from "unsupported on this
/// chain/provider" and "failed to load".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LoadStatus<T> {
    #[default]
    Pending,
    Loaded(T),
    Unsupported,
    Failed(String),
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
    pub asset_changes: LoadStatus<Vec<AssetChange>>,
    pub state_diff: LoadStatus<StateDiff>,
    /// Call tree for the Internal tab (plan 12.6.5). Populated by
    /// `TxTracePort::call_tree`.
    pub call_tree: LoadStatus<CallNode>,
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
            asset_changes: LoadStatus::default(),
            state_diff: LoadStatus::default(),
            call_tree: LoadStatus::default(),
        }
    }
}
