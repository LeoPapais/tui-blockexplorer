//! HTTP signature directory adapter (openchain-shape).
//!
//! Primary backend of the `SignatureDirectoryPort` fallback chain.
//! Speaks the openchain.xyz signature-database API:
//!
//! - Selectors: `GET /signature-database/v1/lookup?function=0xabcd1234`
//! - Topics:    `GET /signature-database/v1/lookup?event=0xDDF2...B3EF`
//!
//! Response shape:
//! ```json
//! {
//!   "ok": true,
//!   "result": {
//!     "function": { "0xabcd1234": [
//!         { "name": "transfer(address,uint256)", "filtered": false }
//!     ] },
//!     "event": { "0x...": [ ... ] }
//!   }
//! }
//! ```
//! We return the first entry whose `filtered` flag is `false`,
//! matching openchain's recommendation for consumer-grade lookups.
//!
//! Although the type lives in `openchain.rs`, it is generic over its
//! base URL and provenance label, so `SamczsunSignatureDirectory`
//! reuses the same wire code by pointing at a different host.
//!
//! See `plan/15-backlog.md` section 3.2 (probe retired the Sourcify
//! mirror; openchain + Samczsun took over) and
//! `.cursor/rules/external-apis.mdc`.

use std::{collections::HashMap, time::Duration};

use reqwest::Client;
use serde::Deserialize;
use thiserror::Error;
use url::Url;

use crate::{
    application::{
        ports::{SignatureDirectoryPort, SignatureHit},
        tx_view::SignatureSource,
    },
    domain::DomainError,
};

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

/// Raw wire response from openchain-shape lookups. The outer `ok`
/// flag is advisory; we only consume the `result` map.
#[derive(Debug, Deserialize)]
struct LookupResponse {
    #[serde(default)]
    result: LookupResult,
}

#[derive(Debug, Default, Deserialize)]
struct LookupResult {
    #[serde(default)]
    function: HashMap<String, Vec<LookupEntry>>,
    #[serde(default)]
    event: HashMap<String, Vec<LookupEntry>>,
}

#[derive(Debug, Deserialize)]
struct LookupEntry {
    name: String,
    /// Openchain marks spam / collision entries with `filtered: true`.
    /// Missing field is treated as `false` for backwards compatibility
    /// with older snapshots.
    #[serde(default)]
    filtered: bool,
}

/// Openchain-shape HTTP adapter for signature lookups.
#[derive(Debug, Clone)]
pub struct HttpSignatureDirectory {
    http: Client,
    base_url: Url,
    source: SignatureSource,
}

impl HttpSignatureDirectory {
    /// Build an adapter pointed at an explicit base URL. Tests swap
    /// in a wiremock URL; the provenance label is captured here so
    /// the port result carries the right [`SignatureSource`].
    #[must_use]
    pub fn new(http: Client, base_url: Url, source: SignatureSource) -> Self {
        Self {
            http,
            base_url,
            source,
        }
    }

    /// Production constructor: 5 s timeout, `https://api.openchain.xyz`,
    /// provenance `SignatureSource::Openchain`.
    pub fn openchain_with_default_http() -> Result<Self, SignatureError> {
        let http = Client::builder().timeout(Duration::from_secs(5)).build()?;
        let url = Url::parse("https://api.openchain.xyz").expect("static openchain URL must parse");
        Ok(Self::new(http, url, SignatureSource::Openchain))
    }

    async fn fetch_first(
        &self,
        query_key: &'static str,
        hex: &str,
    ) -> Result<Option<SignatureHit>, SignatureError> {
        let mut url = self.base_url.clone();
        url.set_path("/signature-database/v1/lookup");
        url.query_pairs_mut().append_pair(query_key, hex);

        let resp = self.http.get(url).send().await?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let body: LookupResponse = resp
            .json()
            .await
            .map_err(|e| SignatureError::Decode(e.to_string()))?;

        let bucket = match query_key {
            "function" => body.result.function.get(hex),
            "event" => body.result.event.get(hex),
            // unreachable: callers pass string literals only
            _ => None,
        };
        let signature = bucket
            .and_then(|entries| entries.iter().find(|e| !e.filtered))
            .map(|entry| entry.name.clone());

        Ok(signature.map(|signature| SignatureHit {
            signature,
            source: self.source,
        }))
    }
}

impl SignatureDirectoryPort for HttpSignatureDirectory {
    async fn lookup_selector(
        &self,
        selector: [u8; 4],
    ) -> Result<Option<SignatureHit>, DomainError> {
        let hex = format!("0x{}", hex::encode(selector));
        self.fetch_first("function", &hex)
            .await
            .map_err(SignatureError::into_domain)
    }

    async fn lookup_event_topic(
        &self,
        topic: [u8; 32],
    ) -> Result<Option<SignatureHit>, DomainError> {
        let hex = format!("0x{}", hex::encode(topic));
        self.fetch_first("event", &hex)
            .await
            .map_err(SignatureError::into_domain)
    }
}
