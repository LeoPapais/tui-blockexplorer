//! Alchemy-backed [`TokenReaderPort`].
//!
//! Composes `alchemy_getTokenMetadata` with an `eth_call` on the
//! ERC-20 `totalSupply()` selector `0x18160ddd`, issued in parallel
//! via `tokio::join!`.
//!
//! See `plan/8-token-detail.md` section 12.2.

use serde::Deserialize;

use super::client::{RpcClient, RpcError};
use crate::{
    application::ports::TokenReaderPort,
    domain::{Address, Chain, DomainError, TokenMetadata, TokenOverview},
};

/// ERC-20 `totalSupply()` selector: keccak256("totalSupply()")[0..4].
const SELECTOR_TOTAL_SUPPLY: &str = "0x18160ddd";

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
pub struct AlchemyTokenReader {
    client: RpcClient,
}

impl AlchemyTokenReader {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }
}

impl TokenReaderPort for AlchemyTokenReader {
    async fn get(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<Option<TokenOverview>, DomainError> {
        let hex = address.to_hex();

        let (metadata_res, supply_res): (
            Result<RawMetadata, RpcError>,
            Result<String, RpcError>,
        ) = tokio::join!(
            self.client
                .call("alchemy_getTokenMetadata", serde_json::json!([hex])),
            self.client.call(
                "eth_call",
                serde_json::json!([
                    { "to": hex, "data": SELECTOR_TOTAL_SUPPLY },
                    "latest"
                ]),
            ),
        );

        let metadata = metadata_res.map_err(|e| e.into_domain())?;
        let (Some(symbol), Some(decimals)) = (metadata.symbol, metadata.decimals) else {
            // Plan section 12.2 calls this the "unsupported token"
            // fallback: without symbol + decimals the screen cannot
            // render a meaningful row, so we surface Ok(None).
            return Ok(None);
        };
        let name = metadata.name.unwrap_or_default();

        let supply_hex = supply_res.map_err(|e| e.into_domain())?;
        let total_supply = parse_u128_word(&supply_hex)?;

        Ok(Some(TokenOverview {
            metadata: TokenMetadata {
                address,
                symbol,
                name,
                decimals,
            },
            total_supply,
        }))
    }
}

fn parse_u128_word(hex: &str) -> Result<u128, DomainError> {
    let stripped = hex.strip_prefix("0x").unwrap_or(hex);
    if stripped.is_empty() {
        return Ok(0);
    }
    if stripped.len() > 64 {
        return Err(DomainError::Internal(format!(
            "totalSupply returned more than 32 bytes: {stripped}"
        )));
    }
    // Take only the low 128 bits (last 32 hex chars). Supplies above
    // 2^128 are real but rare; truncation is documented in the plan.
    let low_start = stripped.len().saturating_sub(32);
    let low = &stripped[low_start..];
    u128::from_str_radix(low, 16)
        .map_err(|e| DomainError::Internal(format!("invalid totalSupply hex: {e}")))
}
