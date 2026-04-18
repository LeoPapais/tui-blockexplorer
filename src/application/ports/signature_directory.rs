//! Outbound port for function / event signature lookups.
//!
//! Primary backend is `api.openchain.xyz`
//! (`HttpSignatureDirectory`) with Samczsun
//! (`SamczsunSignatureDirectory`) as a strict fallback. The two are
//! composed via `CompositeSignatureDirectory` so the application
//! layer sees a single port.
//!
//! See `plan/15-backlog.md` section 3.2 and
//! `plan/4-tx-detail.md` section 12.4.1.

use crate::{application::tx_view::SignatureSource, domain::DomainError};

/// Signature-lookup result. Carries the resolved signature text plus
/// the directory that produced it, so the UI can render the decoded
/// method with its provenance (`Openchain` vs `Samczsun`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHit {
    /// Canonical signature text, e.g. `transfer(address,uint256)`.
    pub signature: String,
    /// Which directory produced this hit. Adapters never emit
    /// [`SignatureSource::Abi`]: that variant is reserved for
    /// ABI-backed decoding in the application layer.
    pub source: SignatureSource,
}

pub trait SignatureDirectoryPort: Send + Sync {
    /// Resolve a 4-byte function selector into a canonical signature
    /// string such as `transfer(address,uint256)`. Returns
    /// `Ok(None)` when the directory has no match.
    fn lookup_selector(
        &self,
        selector: [u8; 4],
    ) -> impl std::future::Future<Output = Result<Option<SignatureHit>, DomainError>> + Send;

    /// Resolve a 32-byte event topic0 into the canonical event
    /// signature text, e.g. `Transfer(address,address,uint256)`.
    fn lookup_event_topic(
        &self,
        topic: [u8; 32],
    ) -> impl std::future::Future<Output = Result<Option<SignatureHit>, DomainError>> + Send;
}
