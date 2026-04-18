//! Etherscan-backed [`TokenSearchPort`] implementation.
//!
//! Two paths:
//!
//! 1. **Ticker lookup** (`by_symbol`) — consults the curated table in
//!    [`super::tickers`] keyed by `(Chain, ticker)`. On a hit the
//!    adapter enriches the entry with `contract/getsourcecode` so the
//!    verified contract name (or a sensible fallback) is surfaced.
//!    Misses return an empty list and hit zero bytes of network.
//! 2. **Free-text lookup** (`by_name`) — scans the same curated table
//!    with a case-insensitive `contains`. Returns every match without
//!    firing any HTTP.
//!
//! Address-shaped inputs fall under `ResolveQuery::Address` and land on
//! the address lookup ports instead of here, by design.
//!
//! See `plan/2-search.md` section 10.2.

use super::{client::EtherscanClient, tickers::tickers_for};
use crate::{
    adapters::etherscan::EtherscanError,
    application::ports::TokenSearchPort,
    domain::{Address, Chain, DomainError, TokenMetadata},
};

#[derive(Debug, Clone)]
pub struct EtherscanTokenSearch {
    client: EtherscanClient,
}

impl EtherscanTokenSearch {
    #[must_use]
    pub fn new(client: EtherscanClient) -> Self {
        Self { client }
    }

    /// Fetch the contract name reported by Etherscan's
    /// `contract/getsourcecode` endpoint. Returns `None` when the
    /// contract is unverified or Etherscan reports an empty
    /// `ContractName`. Errors from the HTTP layer are mapped into
    /// [`DomainError`].
    async fn fetch_contract_name(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<Option<String>, DomainError> {
        let addr_hex = address.to_hex();
        let response = self
            .client
            .get(
                chain,
                "contract",
                "getsourcecode",
                &[("address", addr_hex.as_str())],
            )
            .await
            .map_err(EtherscanError::into_domain)?;

        let array = response
            .get("result")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let Some(first) = array.first() else {
            return Ok(None);
        };
        let name = first
            .get("ContractName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() {
            Ok(None)
        } else {
            Ok(Some(name))
        }
    }
}

impl TokenSearchPort for EtherscanTokenSearch {
    async fn by_symbol(
        &self,
        symbol: &str,
        chain: Chain,
    ) -> Result<Vec<TokenMetadata>, DomainError> {
        let needle = symbol.trim().to_ascii_uppercase();
        let mut out = Vec::new();
        for entry in tickers_for(chain) {
            if entry.ticker.eq_ignore_ascii_case(&needle) {
                let address = Address::from_hex(entry.address_hex).map_err(|e| {
                    DomainError::Internal(format!(
                        "malformed curated ticker entry for {}: {e}",
                        entry.ticker
                    ))
                })?;
                let enriched_name = self
                    .fetch_contract_name(address, chain)
                    .await
                    .unwrap_or(None)
                    .unwrap_or_else(|| entry.name.to_string());
                out.push(TokenMetadata {
                    address,
                    symbol: entry.ticker.to_string(),
                    name: enriched_name,
                    decimals: entry.decimals,
                });
            }
        }
        Ok(out)
    }

    async fn by_name(&self, text: &str, chain: Chain) -> Result<Vec<TokenMetadata>, DomainError> {
        let needle = text.trim().to_ascii_lowercase();
        if needle.is_empty() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in tickers_for(chain) {
            let name_lower = entry.name.to_ascii_lowercase();
            let ticker_lower = entry.ticker.to_ascii_lowercase();
            if name_lower.contains(&needle) || ticker_lower.contains(&needle) {
                let address = Address::from_hex(entry.address_hex).map_err(|e| {
                    DomainError::Internal(format!(
                        "malformed curated ticker entry for {}: {e}",
                        entry.ticker
                    ))
                })?;
                out.push(TokenMetadata {
                    address,
                    symbol: entry.ticker.to_string(),
                    name: entry.name.to_string(),
                    decimals: entry.decimals,
                });
            }
        }
        Ok(out)
    }
}
