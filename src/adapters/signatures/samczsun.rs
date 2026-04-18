//! Samczsun signature-database adapter.
//!
//! Strict fallback behind [`HttpSignatureDirectory`]: the composite
//! only queries Samczsun when openchain returns no match. Samczsun
//! mirrors openchain's request / response shape, so the adapter is a
//! thin wrapper that reuses [`HttpSignatureDirectory`] with a
//! different base URL and a dedicated `SignatureSource::Samczsun`
//! label.
//!
//! Base URL: `https://sig.eth.samczsun.com`.
//! Endpoint: `GET /signature-database/v1/lookup?function=0x...` /
//! `?event=0x...`. Same response shape as openchain. Per task spec
//! the per-request timeout is pinned at 2 s so a dead Samczsun never
//! blocks the tx-detail render budget.
//!
//! See `plan/15-backlog.md` section 3.2 and
//! `.cursor/rules/external-apis.mdc` (fallback chain ABI → openchain
//! → Samczsun → raw).

use std::time::Duration;

use reqwest::Client;
use url::Url;

use crate::{
    adapters::signatures::openchain::{HttpSignatureDirectory, SignatureError},
    application::{
        ports::{SignatureDirectoryPort, SignatureHit},
        tx_view::SignatureSource,
    },
    domain::DomainError,
};

/// Samczsun-backed signature directory. Wraps
/// [`HttpSignatureDirectory`] so it can reuse the openchain wire code
/// while tagging every hit with [`SignatureSource::Samczsun`].
#[derive(Debug, Clone)]
pub struct SamczsunSignatureDirectory {
    inner: HttpSignatureDirectory,
}

impl SamczsunSignatureDirectory {
    /// Build an adapter pointed at an explicit base URL. Used by
    /// wiremock-driven tests. Production callers should prefer
    /// [`SamczsunSignatureDirectory::with_default_http`].
    #[must_use]
    pub fn new(http: Client, base_url: Url) -> Self {
        Self {
            inner: HttpSignatureDirectory::new(http, base_url, SignatureSource::Samczsun),
        }
    }

    /// 2 s per-request timeout, pointed at
    /// `https://sig.eth.samczsun.com`.
    pub fn with_default_http() -> Result<Self, SignatureError> {
        let http = Client::builder().timeout(Duration::from_secs(2)).build()?;
        let url =
            Url::parse("https://sig.eth.samczsun.com").expect("static Samczsun URL must parse");
        Ok(Self::new(http, url))
    }
}

impl SignatureDirectoryPort for SamczsunSignatureDirectory {
    async fn lookup_selector(
        &self,
        selector: [u8; 4],
    ) -> Result<Option<SignatureHit>, DomainError> {
        self.inner.lookup_selector(selector).await
    }

    async fn lookup_event_topic(
        &self,
        topic: [u8; 32],
    ) -> Result<Option<SignatureHit>, DomainError> {
        self.inner.lookup_event_topic(topic).await
    }
}
