//! Alchemy-backed [`PortfolioPort`] adapter.
//!
//! Calls `alchemy_getTokenBalances` to get the raw balances, then
//! fans out to `alchemy_getTokenMetadata` per non-zero holding to
//! pick up symbol / decimals / name. To keep the fan-out bounded we
//! only fetch metadata for the first [`MAX_HOLDINGS`] non-zero
//! entries sorted by raw balance descending.
//!
//! See `plan/6-address-detail.md` section 12.4.2.

use serde::Deserialize;
use serde_json::json;

use super::client::{RpcClient, RpcError};
use crate::{
    application::ports::PortfolioPort,
    domain::{Address, Chain, DomainError, PriceLookup, TokenHolding, TokenMetadata, Wei},
};

/// Upper bound on the number of metadata calls issued per
/// `get_token_balances`. Keeps large wallets responsive.
const MAX_HOLDINGS: usize = 20;

#[derive(Debug, Deserialize)]
struct RawBalancesResult {
    #[serde(rename = "tokenBalances", default)]
    token_balances: Vec<RawTokenBalance>,
}

#[derive(Debug, Deserialize)]
struct RawTokenBalance {
    #[serde(rename = "contractAddress")]
    contract_address: String,
    #[serde(rename = "tokenBalance", default)]
    token_balance: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawMetadata {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    symbol: Option<String>,
    #[serde(default)]
    decimals: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct AlchemyPortfolio {
    client: RpcClient,
}

impl AlchemyPortfolio {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }
}

impl PortfolioPort for AlchemyPortfolio {
    async fn get_token_balances(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<Vec<TokenHolding>, DomainError> {
        let params = json!([address.to_hex(), "erc20"]);
        let balances: RawBalancesResult = self
            .client
            .call("alchemy_getTokenBalances", params)
            .await
            .map_err(|e: RpcError| match e {
                RpcError::Rpc { code: -32601, .. } => DomainError::FeatureUnavailable,
                other => other.into_domain(),
            })?;

        // Parse raw balances; drop zero / invalid entries and sort
        // descending to keep the most relevant holdings on top.
        let mut parsed: Vec<(Address, u128)> = balances
            .token_balances
            .into_iter()
            .filter_map(|row| {
                let addr = Address::from_hex(&row.contract_address).ok()?;
                let balance = parse_balance(row.token_balance.as_deref())?;
                if balance == 0 {
                    return None;
                }
                Some((addr, balance))
            })
            .collect();
        parsed.sort_by_key(|(_, b)| std::cmp::Reverse(*b));
        parsed.truncate(MAX_HOLDINGS);

        if parsed.is_empty() {
            return Ok(Vec::new());
        }

        // Fan-in via JSON-RPC batch: one request for the whole list
        // of `alchemy_getTokenMetadata` calls instead of N spawned
        // tasks. See `plan/13-alchemy-adapter.md` §8.5.
        let metadata_params: Vec<[String; 1]> =
            parsed.iter().map(|(addr, _)| [addr.to_hex()]).collect();
        let metadata_results: Vec<Result<RawMetadata, RpcError>> = if metadata_params.len() == 1 {
            // Skip the array wrapper for the degenerate one-call case:
            // keeps the body identical to the pre-batch shape and
            // avoids paying allocation overhead for nothing.
            vec![
                self.client
                    .call::<_, RawMetadata>(
                        "alchemy_getTokenMetadata",
                        json!([metadata_params[0][0]]),
                    )
                    .await,
            ]
        } else {
            match self
                .client
                .call_batch::<_, RawMetadata>("alchemy_getTokenMetadata", &metadata_params)
                .await
            {
                Ok(results) => results,
                Err(_) => {
                    // Batch envelope failed (transport / rate limit):
                    // degrade to per-call best-effort.
                    let mut out = Vec::with_capacity(metadata_params.len());
                    for [addr_hex] in &metadata_params {
                        out.push(
                            self.client
                                .call::<_, RawMetadata>(
                                    "alchemy_getTokenMetadata",
                                    json!([addr_hex]),
                                )
                                .await,
                        );
                    }
                    out
                }
            }
        };

        let mut holdings = Vec::with_capacity(parsed.len());
        for ((contract, balance), meta_result) in parsed.into_iter().zip(metadata_results) {
            // Best-effort metadata: a missing symbol / decimals is
            // rendered as the raw contract address.
            let raw_meta = meta_result.unwrap_or(RawMetadata {
                name: None,
                symbol: None,
                decimals: None,
            });
            holdings.push(TokenHolding {
                metadata: TokenMetadata {
                    address: contract,
                    name: raw_meta.name.unwrap_or_else(|| contract.to_hex()),
                    symbol: raw_meta.symbol.unwrap_or_else(|| "???".to_string()),
                    decimals: raw_meta.decimals.unwrap_or(0),
                },
                balance: Wei::new(balance),
                // Prices are fetched separately by
                // `load_address_portfolio`; this adapter only
                // produces raw balances. See
                // `plan/15-backlog.md` §3.4.
                price: PriceLookup::Pending,
            });
        }
        Ok(holdings)
    }
}

fn parse_balance(value: Option<&str>) -> Option<u128> {
    let value = value?;
    let stripped = value.strip_prefix("0x").unwrap_or(value);
    if stripped.is_empty() {
        return None;
    }
    u128::from_str_radix(stripped, 16).ok()
}
