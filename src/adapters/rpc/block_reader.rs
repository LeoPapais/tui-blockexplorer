//! Alchemy-backed [`BlockReaderPort`].
//!
//! Issues `eth_getBlockByNumber` / `eth_getBlockByHash` with
//! `fullTransactions=false` and maps the wire format into the full
//! `Block` domain entity. See `plan/3-block-detail.md` section 11.2.

use serde::Deserialize;

use super::client::{RpcClient, parse_hex_u128, parse_hex_u64};
use crate::{
    application::ports::BlockReaderPort,
    domain::{
        Address, Block, BlockHash, BlockId, BlockNumber, Chain, DomainError, TxHash,
        UnixTimestamp, Wei,
    },
};

#[derive(Debug, Deserialize)]
struct RawBlock {
    number: String,
    hash: String,
    #[serde(rename = "parentHash")]
    parent_hash: String,
    timestamp: String,
    miner: String,
    #[serde(rename = "gasUsed")]
    gas_used: String,
    #[serde(rename = "gasLimit")]
    gas_limit: String,
    #[serde(rename = "baseFeePerGas", default)]
    base_fee_per_gas: Option<String>,
    size: String,
    #[serde(rename = "extraData", default)]
    extra_data: String,
    #[serde(default)]
    transactions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AlchemyBlockReader {
    client: RpcClient,
}

impl AlchemyBlockReader {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }

    async fn fetch(&self, id: BlockId) -> Result<Option<RawBlock>, DomainError> {
        match id {
            BlockId::Number(n) => self
                .client
                .call(
                    "eth_getBlockByNumber",
                    serde_json::json!([format!("0x{:x}", n.value()), false]),
                )
                .await
                .map_err(|e| e.into_domain()),
            BlockId::Hash(h) => self
                .client
                .call(
                    "eth_getBlockByHash",
                    serde_json::json!([h.to_hex(), false]),
                )
                .await
                .map_err(|e| e.into_domain()),
        }
    }

    fn raw_to_block(raw: RawBlock, chain: Chain) -> Result<Block, DomainError> {
        let number = BlockNumber::new(parse_hex_u64(&raw.number).map_err(|e| e.into_domain())?);
        let hash = BlockHash::from_hex(&raw.hash)?;
        let parent_hash = BlockHash::from_hex(&raw.parent_hash)?;
        let timestamp = UnixTimestamp::from_seconds(
            parse_hex_u64(&raw.timestamp).map_err(|e| e.into_domain())?,
        );
        let miner = Address::from_hex(&raw.miner)?;
        let gas_used = parse_hex_u64(&raw.gas_used).map_err(|e| e.into_domain())?;
        let gas_limit = parse_hex_u64(&raw.gas_limit).map_err(|e| e.into_domain())?;
        let base_fee = match raw.base_fee_per_gas.as_deref() {
            Some(hex) if !hex.is_empty() => Some(Wei::new(
                parse_hex_u128(hex).map_err(|e| e.into_domain())?,
            )),
            _ => None,
        };
        let size = parse_hex_u64(&raw.size).map_err(|e| e.into_domain())?;
        let extra_data = parse_hex_bytes(&raw.extra_data)?;

        let tx_hashes = raw
            .transactions
            .into_iter()
            .map(|h| TxHash::from_hex(&h))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Block {
            chain,
            number,
            hash,
            parent_hash,
            timestamp,
            miner,
            gas_used,
            gas_limit,
            base_fee,
            size,
            extra_data,
            tx_hashes,
        })
    }
}

impl BlockReaderPort for AlchemyBlockReader {
    async fn get(&self, id: BlockId, chain: Chain) -> Result<Option<Block>, DomainError> {
        let Some(raw) = self.fetch(id).await? else {
            return Ok(None);
        };
        Self::raw_to_block(raw, chain).map(Some)
    }
}

fn parse_hex_bytes(s: &str) -> Result<Vec<u8>, DomainError> {
    let stripped = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    if stripped.is_empty() {
        return Ok(Vec::new());
    }
    // `hex::decode` requires an even-length string; pad with a
    // leading zero if needed, matching RPC conventions.
    let owned;
    let input: &str = if stripped.len().is_multiple_of(2) {
        stripped
    } else {
        owned = format!("0{stripped}");
        owned.as_str()
    };
    hex::decode(input).map_err(|e| DomainError::InvalidInput(format!("invalid hex: {e}")))
}
