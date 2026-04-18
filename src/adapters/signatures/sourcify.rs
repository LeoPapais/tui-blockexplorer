//! Sourcify 4byte signature directory adapter.
//!
//! Hits `https://api.4byte.sourcify.dev` which speaks the same
//! 4byte.directory-style API as openchain.xyz. The plan 4 expansion
//! picked this mirror per the user.
//!
//! Endpoint shape (function selectors):
//!   GET /api/v1/signatures?hex_signature=0xabcd1234
//! Response:
//!   {"count": N, "results": [{"text_signature": "...", ...}, ...]}
//!
//! Event topics use the parallel `event-signatures` endpoint with a
//! full 32-byte hex value.
//!
//! See `plan/4-tx-detail.md` section 12.4.1.

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use thiserror::Error;
use url::Url;

use crate::{application::ports::SignatureDirectoryPort, domain::DomainError};

#[derive(Debug, Error)]
pub enum SignatureError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("decode error: {0}")]
    Decode(String),
}

impl SignatureError {
    pub fn into_domain(self) -> DomainError {
        match self {
            SignatureError::Http(err) if err.is_timeout() || err.is_connect() => {
                DomainError::ProviderUnavailable
            }
            SignatureError::Http(err) => DomainError::Internal(err.to_string()),
            SignatureError::Decode(msg) => DomainError::Internal(msg),
        }
    }
}

#[derive(Debug, Deserialize)]
struct PagedResponse {
    #[serde(default)]
    results: Vec<SignatureRow>,
}

#[derive(Debug, Deserialize)]
struct SignatureRow {
    text_signature: String,
}

#[derive(Debug, Clone)]
pub struct SourcifySignatureDirectory {
    http: Client,
    base_url: Url,
}

impl SourcifySignatureDirectory {
    /// Build an adapter pointed at an explicit base URL. Useful in
    /// tests that swap in a wiremock URL; production callers should
    /// prefer [`SourcifySignatureDirectory::with_default_http`].
    #[must_use]
    pub fn new(http: Client, base_url: Url) -> Self {
        Self { http, base_url }
    }

    pub fn with_default_http() -> Result<Self, SignatureError> {
        let http = Client::builder().timeout(Duration::from_secs(5)).build()?;
        let url =
            Url::parse("https://api.4byte.sourcify.dev").expect("static Sourcify URL must parse");
        Ok(Self::new(http, url))
    }

    async fn fetch_first(&self, path: &str, hex: &str) -> Result<Option<String>, SignatureError> {
        let mut url = self.base_url.clone();
        url.set_path(path);
        url.query_pairs_mut().append_pair("hex_signature", hex);

        let resp = self.http.get(url).send().await?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let body: PagedResponse = resp
            .json()
            .await
            .map_err(|e| SignatureError::Decode(e.to_string()))?;
        Ok(body.results.into_iter().next().map(|r| r.text_signature))
    }
}

impl SignatureDirectoryPort for SourcifySignatureDirectory {
    async fn lookup_selector(&self, selector: [u8; 4]) -> Result<Option<String>, DomainError> {
        let hex = format!("0x{}", hex::encode(selector));
        self.fetch_first("/api/v1/signatures", &hex)
            .await
            .map_err(SignatureError::into_domain)
    }

    async fn lookup_event_topic(&self, topic: [u8; 32]) -> Result<Option<String>, DomainError> {
        let hex = format!("0x{}", hex::encode(topic));
        self.fetch_first("/api/v1/event-signatures", &hex)
            .await
            .map_err(SignatureError::into_domain)
    }
}
