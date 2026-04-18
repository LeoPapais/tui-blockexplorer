//! Alchemy-backed [`BlockLookupPort`].
//!
//! See `plan/2-search.md` section 10.2.

use serde::Deserialize;

use super::client::{RpcClient, parse_hex_u64};
use crate::{
    application::ports::BlockLookupPort,
    domain::{BlockHash, BlockNumber, BlockSummary, Chain, DomainError},
};

#[derive(Debug, Deserialize)]
struct RawBlock {
    number: String,
    hash: String,
}

#[derive(Debug, Clone)]
pub struct AlchemyBlockLookup {
    client: RpcClient,
}

impl AlchemyBlockLookup {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }

    fn raw_to_summary(raw: Option<RawBlock>) -> Result<Option<BlockSummary>, DomainError> {
        let Some(raw) = raw else { return Ok(None) };
        Ok(Some(BlockSummary {
            number: BlockNumber::new(parse_hex_u64(&raw.number).map_err(|e| e.into_domain())?),
            hash: BlockHash::from_hex(&raw.hash)?,
        }))
    }
}

impl BlockLookupPort for AlchemyBlockLookup {
    async fn get_by_hash(
        &self,
        hash: BlockHash,
        _chain: Chain,
    ) -> Result<Option<BlockSummary>, DomainError> {
        let raw: Option<RawBlock> = self
            .client
            .call(
                "eth_getBlockByHash",
                serde_json::json!([hash.to_hex(), false]),
            )
            .await
            .map_err(|e| e.into_domain())?;
        Self::raw_to_summary(raw)
    }

    async fn get_by_number(
        &self,
        number: BlockNumber,
        _chain: Chain,
    ) -> Result<Option<BlockSummary>, DomainError> {
        let raw: Option<RawBlock> = self
            .client
            .call(
                "eth_getBlockByNumber",
                serde_json::json!([format!("0x{:x}", number.value()), false]),
            )
            .await
            .map_err(|e| e.into_domain())?;
        Self::raw_to_summary(raw)
    }
}
