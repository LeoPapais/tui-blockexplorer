//! Thin HTTP client for Etherscan V2 endpoints.
//!
//! Every V2 endpoint lives at a single root URL
//! (`https://api.etherscan.io/v2/api`) and discriminates chains via
//! the `chainid` query parameter. See the introduction at
//! <https://docs.etherscan.io/introduction>.

use std::time::Duration;

use reqwest::Client;
use thiserror::Error;
use url::Url;

use crate::domain::{Chain, DomainError};

#[derive(Debug, Error)]
pub enum EtherscanError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("response missing `result` field")]
    MissingResult,

    #[error("decode error: {0}")]
    Decode(String),

    #[error("etherscan error: {0}")]
    Api(String),
}

impl EtherscanError {
    pub fn into_domain(self) -> DomainError {
        match self {
            EtherscanError::Http(err) if err.is_timeout() => DomainError::ProviderUnavailable,
            EtherscanError::Http(err) if err.is_connect() => DomainError::ProviderUnavailable,
            EtherscanError::Http(err) => DomainError::Internal(err.to_string()),
            EtherscanError::MissingResult => {
                DomainError::Internal("missing result field".to_string())
            }
            EtherscanError::Decode(msg) => DomainError::Internal(msg),
            EtherscanError::Api(msg) => DomainError::Internal(msg),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EtherscanClient {
    http: Client,
    base_url: Url,
    api_key: String,
}

impl EtherscanClient {
    #[must_use]
    pub fn new(base_url: Url, api_key: String, http: Client) -> Self {
        Self {
            http,
            base_url,
            api_key,
        }
    }

    /// Convenience constructor that wires the default Etherscan V2
    /// base URL and a 10-second timeout.
    pub fn with_default_http(api_key: String) -> Result<Self, EtherscanError> {
        let http = Client::builder().timeout(Duration::from_secs(10)).build()?;
        let url = Url::parse("https://api.etherscan.io/v2/api")
            .expect("static Etherscan URL must parse");
        Ok(Self::new(url, api_key, http))
    }

    /// Issue a GET to the shared V2 endpoint with the common
    /// (`module`, `action`, `chainid`, `apikey`) params, merged with
    /// the caller-supplied `extra` pairs. Returns the raw response
    /// body JSON.
    pub async fn get(
        &self,
        chain: Chain,
        module: &str,
        action: &str,
        extra: &[(&str, &str)],
    ) -> Result<serde_json::Value, EtherscanError> {
        let mut url = self.base_url.clone();
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("chainid", &chain_id_for(chain).to_string())
                .append_pair("module", module)
                .append_pair("action", action)
                .append_pair("apikey", &self.api_key);
            for (key, value) in extra {
                query.append_pair(key, value);
            }
        }
        let resp = self.http.get(url).send().await?;
        let body: serde_json::Value = resp.json().await?;
        Ok(body)
    }
}

/// Return the numeric chain id expected by Etherscan V2's
/// `chainid` parameter. Mirrors `Chain::alchemy_subdomain` in
/// spirit but uses the canonical decimal chain IDs.
#[must_use]
pub const fn chain_id_for(chain: Chain) -> u64 {
    match chain {
        Chain::Ethereum => 1,
        Chain::EthereumSepolia => 11_155_111,
        Chain::Base => 8_453,
        Chain::Polygon => 137,
        Chain::Optimism => 10,
        Chain::Arbitrum => 42_161,
    }
}
