//! Composite signature directory.
//!
//! Wires openchain (primary) and Samczsun (fallback) behind a single
//! [`SignatureDirectoryPort`]. The primary is queried first; the
//! fallback only runs when the primary returns `Ok(None)`. Errors
//! from the primary are forwarded: a failing primary short-circuits
//! the lookup rather than silently masking infrastructure problems.
//!
//! The returned [`SignatureHit`] keeps the provenance set by the
//! underlying adapter (`SignatureSource::Openchain` or
//! `SignatureSource::Samczsun`), so the UI can render "from openchain"
//! vs "from samczsun" under the decoded method.
//!
//! See `plan/15-backlog.md` section 3.2 and
//! `.cursor/rules/external-apis.mdc`.

use crate::{
    application::ports::{SignatureDirectoryPort, SignatureHit},
    domain::DomainError,
};

/// Generic composite. Two ports, primary then fallback. Kept generic
/// (rather than `Arc<dyn ...>`) so hot-path callers stay monomorphic
/// and the fallback can be any pair of ports in tests.
#[derive(Debug, Clone)]
pub struct CompositeSignatureDirectory<Primary, Fallback> {
    primary: Primary,
    fallback: Fallback,
}

impl<Primary, Fallback> CompositeSignatureDirectory<Primary, Fallback> {
    #[must_use]
    pub fn new(primary: Primary, fallback: Fallback) -> Self {
        Self { primary, fallback }
    }
}

impl<Primary, Fallback> SignatureDirectoryPort for CompositeSignatureDirectory<Primary, Fallback>
where
    Primary: SignatureDirectoryPort,
    Fallback: SignatureDirectoryPort,
{
    async fn lookup_selector(
        &self,
        selector: [u8; 4],
    ) -> Result<Option<SignatureHit>, DomainError> {
        if let Some(hit) = self.primary.lookup_selector(selector).await? {
            return Ok(Some(hit));
        }
        self.fallback.lookup_selector(selector).await
    }

    async fn lookup_event_topic(
        &self,
        topic: [u8; 32],
    ) -> Result<Option<SignatureHit>, DomainError> {
        if let Some(hit) = self.primary.lookup_event_topic(topic).await? {
            return Ok(Some(hit));
        }
        self.fallback.lookup_event_topic(topic).await
    }
}
