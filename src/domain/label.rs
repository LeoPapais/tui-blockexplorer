//! Human-readable label attached to an on-chain address.
//!
//! Threaded into the Block Detail Overview tab so the miner / fee
//! recipient row renders `0xab…cd  (Coinbase)` and §3.5 of
//! `plan/15-backlog.md` can upgrade the raw Polygon signer row to
//! "signer 0x…  (Polygon: validator 7)".
//!
//! See `plan/3-block-detail.md` §12.4.

/// Where the label came from. Exposed so the UI can hint at the
/// provenance (for example a muted "well-known" tag vs a plain
/// label when sourced from Etherscan).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelSource {
    /// Bundled in the binary via a static table of well-known
    /// addresses per chain (validators, exchange hot wallets, ...).
    WellKnown,
    /// Looked up live from Etherscan's `getsourcecode` endpoint
    /// (`ContractName` field).
    Etherscan,
}

/// Label + where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub name: String,
    pub source: LabelSource,
}

impl Label {
    /// Convenience: build a well-known label.
    #[must_use]
    pub fn well_known(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: LabelSource::WellKnown,
        }
    }

    /// Convenience: build an Etherscan-sourced label.
    #[must_use]
    pub fn etherscan(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: LabelSource::Etherscan,
        }
    }
}
