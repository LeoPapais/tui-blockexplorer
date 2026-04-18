//! Alchemy-backed [`BlockReceiptsPort`].
//!
//! Issues `eth_getBlockByNumber(fullTransactions=true)` and
//! `eth_getBlockReceipts` in parallel, zips the two by index and
//! maps the result onto `Vec<BlockTxReceipt>`. Categorisation is
//! computed inside the mapping via `TxCategory::classify` so the
//! adapter ships ready-to-render rows to the application layer.
//!
//! See `plan/3-block-detail.md` §12.3.

use serde::Deserialize;

use super::client::{RpcClient, parse_hex_u64};
use crate::{
    application::ports::BlockReceiptsPort,
    domain::{Address, BlockId, BlockTxReceipt, Chain, DomainError, TxHash, TxStatus, Wei},
};

#[derive(Debug, Deserialize)]
struct RawFullBlock {
    transactions: Vec<RawFullTx>,
}

#[derive(Debug, Deserialize)]
struct RawFullTx {
    hash: String,
    #[serde(rename = "transactionIndex")]
    tx_index: String,
    from: String,
    #[serde(default)]
    to: Option<String>,
    value: String,
    #[serde(default)]
    input: String,
}

#[derive(Debug, Deserialize)]
struct RawReceipt {
    #[serde(rename = "transactionHash")]
    tx_hash: String,
    #[serde(rename = "gasUsed")]
    gas_used: String,
    status: String,
    #[serde(default, rename = "contractAddress")]
    contract_address: Option<String>,
    #[serde(default, rename = "revertReason")]
    revert_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AlchemyBlockReceipts {
    client: RpcClient,
}

impl AlchemyBlockReceipts {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }

    fn id_to_params(id: BlockId) -> serde_json::Value {
        match id {
            BlockId::Number(n) => serde_json::json!([format!("0x{:x}", n.value())]),
            BlockId::Hash(h) => serde_json::json!([h.to_hex()]),
        }
    }

    fn full_block_method(id: BlockId) -> &'static str {
        match id {
            BlockId::Number(_) => "eth_getBlockByNumber",
            BlockId::Hash(_) => "eth_getBlockByHash",
        }
    }

    fn full_block_params(id: BlockId) -> serde_json::Value {
        match id {
            BlockId::Number(n) => serde_json::json!([format!("0x{:x}", n.value()), true]),
            BlockId::Hash(h) => serde_json::json!([h.to_hex(), true]),
        }
    }

    fn zip_rows(
        full: RawFullBlock,
        receipts: Vec<RawReceipt>,
    ) -> Result<Vec<BlockTxReceipt>, DomainError> {
        let mut by_hash: std::collections::HashMap<String, RawReceipt> =
            std::collections::HashMap::with_capacity(receipts.len());
        for r in receipts {
            by_hash.insert(r.tx_hash.to_lowercase(), r);
        }

        let mut out = Vec::with_capacity(full.transactions.len());
        for tx in full.transactions {
            let Some(receipt) = by_hash.remove(&tx.hash.to_lowercase()) else {
                return Err(DomainError::Internal(format!(
                    "missing receipt for tx {}",
                    tx.hash
                )));
            };
            out.push(merge_row(tx, receipt)?);
        }
        Ok(out)
    }
}

fn merge_row(tx: RawFullTx, receipt: RawReceipt) -> Result<BlockTxReceipt, DomainError> {
    let hash = TxHash::from_hex(&tx.hash)?;
    let tx_index = parse_hex_u64(&tx.tx_index).map_err(|e| e.into_domain())?;
    let from = Address::from_hex(&tx.from)?;
    let to = match tx.to.as_deref() {
        Some(s) if !s.is_empty() && s != "null" => Some(Address::from_hex(s)?),
        _ => None,
    };
    let value = Wei::new(parse_hex_u128(&tx.value)?);
    let input = parse_hex_bytes(&tx.input)?;

    let gas_used = parse_hex_u64(&receipt.gas_used).map_err(|e| e.into_domain())?;
    let status = match receipt.status.as_str() {
        "0x1" => TxStatus::Success,
        _ => TxStatus::Failed {
            reason: receipt.revert_reason.clone(),
        },
    };
    let contract_address = match receipt.contract_address.as_deref() {
        Some(s) if !s.is_empty() && s != "null" && s != "0x" => Some(Address::from_hex(s)?),
        _ => None,
    };

    Ok(BlockTxReceipt::from_fields(
        hash,
        tx_index,
        from,
        to,
        value,
        &input,
        Some(gas_used),
        status,
        contract_address,
    ))
}

fn parse_hex_u128(s: &str) -> Result<u128, DomainError> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    // An empty quantity is valid hex-encoded zero per the RPC
    // standard ("0x" alone). Treat it as zero.
    if stripped.is_empty() {
        return Ok(0);
    }
    u128::from_str_radix(stripped, 16)
        .map_err(|e| DomainError::Internal(format!("invalid hex: {e}")))
}

fn parse_hex_bytes(s: &str) -> Result<Vec<u8>, DomainError> {
    let stripped = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    if stripped.is_empty() {
        return Ok(Vec::new());
    }
    let owned;
    let input: &str = if stripped.len().is_multiple_of(2) {
        stripped
    } else {
        owned = format!("0{stripped}");
        owned.as_str()
    };
    hex::decode(input).map_err(|e| DomainError::InvalidInput(format!("invalid hex: {e}")))
}

impl BlockReceiptsPort for AlchemyBlockReceipts {
    async fn get_transactions(
        &self,
        id: BlockId,
        _chain: Chain,
    ) -> Result<Vec<BlockTxReceipt>, DomainError> {
        let (full_res, receipts_res) =
            tokio::join!(
                self.client.call::<_, Option<RawFullBlock>>(
                    Self::full_block_method(id),
                    Self::full_block_params(id),
                ),
                self.client.call::<_, Option<Vec<RawReceipt>>>(
                    "eth_getBlockReceipts",
                    Self::id_to_params(id),
                ),
            );

        let full = full_res.map_err(|e| e.into_domain())?;
        let receipts = receipts_res.map_err(|e| e.into_domain())?;

        let full = full.ok_or(DomainError::NotFound)?;
        let receipts = receipts.unwrap_or_default();

        Self::zip_rows(full, receipts)
    }
}
