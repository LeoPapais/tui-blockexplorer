//! Alchemy-backed [`TxLookupPort`].
//!
//! See `plan/2-search.md` section 10.2.

use serde::Deserialize;

use super::client::{RpcClient, parse_hex_u64};
use crate::{
    application::ports::TxLookupPort,
    domain::{BlockNumber, Chain, DomainError, TxHash, TxSummary},
};

#[derive(Debug, Deserialize)]
struct RawTx {
    hash: String,
    #[serde(rename = "blockNumber", default)]
    block_number: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AlchemyTxLookup {
    client: RpcClient,
}

impl AlchemyTxLookup {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }
}

impl TxLookupPort for AlchemyTxLookup {
    async fn get(&self, hash: TxHash, _chain: Chain) -> Result<Option<TxSummary>, DomainError> {
        let resp: Option<RawTx> = self
            .client
            .call(
                "eth_getTransactionByHash",
                serde_json::json!([hash.to_hex()]),
            )
            .await
            .map_err(|e| e.into_domain())?;

        let Some(raw) = resp else {
            return Ok(None);
        };

        let hash = TxHash::from_hex(&raw.hash)?;
        let block = match raw.block_number {
            Some(ref hex) if !hex.is_empty() => Some(BlockNumber::new(
                parse_hex_u64(hex).map_err(|e| e.into_domain())?,
            )),
            _ => None,
        };
        Ok(Some(TxSummary { hash, block }))
    }
}
