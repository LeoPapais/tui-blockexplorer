//! Thin HTTP client for the Alchemy Prices API.
//!
//! The Prices API is REST, not JSON-RPC: every endpoint is a POST
//! with a JSON body that already carries the network/address pair.
//! See <https://www.alchemy.com/docs/data> → Prices API.

use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use url::Url;

use crate::domain::DomainError;

/// Errors produced by the Prices REST client. Mapped to
/// [`DomainError`] at the adapter boundary via
/// [`PricesError::into_domain`].
#[derive(Debug, Error)]
pub enum PricesError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),

    #[error("rate limited")]
    Rate,

    /// The Alchemy Prices API answered 404 for the requested token,
    /// i.e. the provider has no price data for that contract. See
    /// `plan/15-backlog.md` §3.4 for the fallback contract.
    #[error("token not indexed by prices provider")]
    NotIndexed,

    #[error("prices API returned {status}: {body}")]
    Api { status: u16, body: String },
}

impl PricesError {
    pub fn into_domain(self) -> DomainError {
        match self {
            PricesError::Rate => DomainError::ProviderUnavailable,
            PricesError::Http(err) if err.is_timeout() => DomainError::ProviderUnavailable,
            PricesError::Http(err) if err.is_connect() => DomainError::ProviderUnavailable,
            PricesError::Http(err) => DomainError::Internal(err.to_string()),
            PricesError::Decode(err) => DomainError::Internal(err.to_string()),
            PricesError::NotIndexed => {
                DomainError::Internal("token not indexed by prices provider".into())
            }
            // HTTP 5xx from Alchemy Prices is a provider outage, not
            // a caller bug. Map it to `ProviderUnavailable` so the
            // breaker + degraded-state UX both treat it like a
            // transient infrastructure blip. See
            // `plan/15-backlog.md` §8.16 "Global error fixtures".
            PricesError::Api { status, .. } if (500..600).contains(&status) => {
                DomainError::ProviderUnavailable
            }
            PricesError::Api { status, body } => {
                DomainError::Internal(format!("prices API {status}: {body}"))
            }
        }
    }
}

/// REST client over the Prices API. Holds a shared
/// [`reqwest::Client`] and the pre-built base URL
/// (`https://api.g.alchemy.com/prices/v1/{api_key}`). Method paths
/// are appended by individual endpoint helpers.
#[derive(Debug, Clone)]
pub struct PricesClient {
    http: Client,
    base_url: Url,
}

impl PricesClient {
    /// Build a client with an explicit base URL. Used by the
    /// wiremock tests and, in live mode, by
    /// [`Self::with_api_key`].
    #[must_use]
    pub fn new(base_url: Url, http: Client) -> Self {
        Self { http, base_url }
    }

    /// Convenience constructor used from `infra::wiring`. Produces
    /// a client with a 10-second timeout pointing at the canonical
    /// Alchemy host.
    pub fn with_api_key(api_key: &str) -> Result<Self, PricesError> {
        let url = Url::parse(&format!("https://api.g.alchemy.com/prices/v1/{api_key}/")).map_err(
            |e| PricesError::Api {
                status: 0,
                body: format!("invalid base URL: {e}"),
            },
        )?;
        let http = Client::builder().timeout(Duration::from_secs(10)).build()?;
        Ok(Self::new(url, http))
    }

    /// Issue a POST with `body` serialised as JSON to
    /// `{base_url}{path}`. `path` must not include a leading slash.
    pub async fn post<B: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<R, PricesError> {
        let url = self.base_url.join(path).map_err(|e| PricesError::Api {
            status: 0,
            body: format!("invalid path: {e}"),
        })?;

        let resp = self.http.post(url).json(body).send().await?;
        let status = resp.status();

        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(PricesError::Rate);
        }

        // plan/15-backlog.md §3.4: a 404 means the Alchemy Prices
        // API has no data for this contract. Treat it as a first-
        // class signal rather than a generic Api error so the
        // adapter can map it to `PriceLookup::Unsupported`.
        if status == StatusCode::NOT_FOUND {
            return Err(PricesError::NotIndexed);
        }

        let body_text = resp.text().await?;

        if !status.is_success() {
            return Err(PricesError::Api {
                status: status.as_u16(),
                body: body_text,
            });
        }

        serde_json::from_str::<R>(&body_text).map_err(PricesError::Decode)
    }
}
