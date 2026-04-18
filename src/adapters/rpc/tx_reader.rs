//! Alchemy-backed [`TxReaderPort`].
//!
//! Issues `eth_getTransactionByHash` + `eth_getTransactionReceipt` in
//! parallel with `tokio::join!` and merges the two into a full
//! [`Transaction`] entity. The Raw tab backing is populated from the
//! pretty-printed tx JSON body so users can inspect the exact
//! response shape.
//!
//! Pending transactions (null receipt / null blockNumber) are
//! supported: block-level fields stay `None` and the status becomes
//! [`TxStatus::Pending`]. See `plan/4-tx-detail.md` section 12.4.2.

use serde::Deserialize;
use serde_json::Value;

use super::client::{RpcClient, RpcError, parse_hex_u128, parse_hex_u64};
use crate::{
    application::ports::TxReaderPort,
    domain::{
        Address, BlockHash, BlockNumber, Chain, DomainError, LogEntry, Transaction, TxHash,
        TxStatus, TxType, Wei,
    },
};

#[derive(Debug, Deserialize)]
struct RawTx {
    hash: String,
    #[serde(rename = "blockNumber", default)]
    block_number: Option<String>,
    #[serde(rename = "blockHash", default)]
    block_hash: Option<String>,
    #[serde(rename = "transactionIndex", default)]
    tx_index: Option<String>,
    from: String,
    #[serde(default)]
    to: Option<String>,
    value: String,
    #[serde(rename = "gasPrice")]
    gas_price: String,
    gas: String,
    nonce: String,
    #[serde(default, rename = "type")]
    tx_type: Option<String>,
    #[serde(default)]
    input: String,
}

#[derive(Debug, Deserialize)]
struct RawReceipt {
    #[serde(rename = "gasUsed")]
    gas_used: String,
    status: String,
    #[serde(default, rename = "revertReason")]
    revert_reason: Option<String>,
    #[serde(default)]
    logs: Vec<RawLog>,
}

#[derive(Debug, Deserialize)]
struct RawLog {
    address: String,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    data: String,
}

#[derive(Debug, Clone)]
pub struct AlchemyTxReader {
    client: RpcClient,
}

impl AlchemyTxReader {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }
}

impl TxReaderPort for AlchemyTxReader {
    async fn get(
        &self,
        hash: TxHash,
        chain: Chain,
    ) -> Result<Option<Transaction>, DomainError> {
        let hash_hex = hash.to_hex();

        let (tx_res, receipt_res): (
            Result<Option<Value>, RpcError>,
            Result<Option<RawReceipt>, RpcError>,
        ) = tokio::join!(
            self.client.call(
                "eth_getTransactionByHash",
                serde_json::json!([hash_hex]),
            ),
            self.client
                .call("eth_getTransactionReceipt", serde_json::json!([hash_hex])),
        );

        let tx_value = tx_res.map_err(|e| e.into_domain())?;
        let receipt = receipt_res.map_err(|e| e.into_domain())?;

        let Some(tx_value) = tx_value else {
            return Ok(None);
        };

        let raw_json = serde_json::to_string_pretty(&tx_value)
            .unwrap_or_else(|_| tx_value.to_string());
        let raw_tx: RawTx = serde_json::from_value(tx_value)
            .map_err(|e| DomainError::Internal(format!("tx decode: {e}")))?;

        let tx_hash = TxHash::from_hex(&raw_tx.hash)?;
        let block_number = opt_block_number(raw_tx.block_number.as_deref())?;
        let block_hash = opt_block_hash(raw_tx.block_hash.as_deref())?;
        let tx_index = opt_hex_u64(raw_tx.tx_index.as_deref())?;
        let from = Address::from_hex(&raw_tx.from)?;
        let to = match raw_tx.to.as_deref() {
            Some(hex) if !hex.is_empty() && hex != "0x" => Some(Address::from_hex(hex)?),
            _ => None,
        };
        let value = Wei::new(parse_hex_u128(&raw_tx.value).map_err(|e| e.into_domain())?);
        let gas_price =
            Wei::new(parse_hex_u128(&raw_tx.gas_price).map_err(|e| e.into_domain())?);
        let gas_limit = parse_hex_u64(&raw_tx.gas).map_err(|e| e.into_domain())?;
        let nonce = parse_hex_u64(&raw_tx.nonce).map_err(|e| e.into_domain())?;
        let tx_type = raw_tx
            .tx_type
            .as_deref()
            .map(TxType::from_hex)
            .unwrap_or(TxType::Legacy);
        let input = parse_hex_bytes(&raw_tx.input)?;

        let (status, gas_used, logs) = match receipt {
            Some(receipt) => {
                let gas_used = parse_hex_u64(&receipt.gas_used).map_err(|e| e.into_domain())?;
                let status = if parse_hex_u64(&receipt.status).unwrap_or(0) == 1 {
                    TxStatus::Success
                } else {
                    TxStatus::Failed {
                        reason: receipt.revert_reason,
                    }
                };
                let logs = receipt
                    .logs
                    .into_iter()
                    .map(raw_log_to_entry)
                    .collect::<Result<Vec<_>, _>>()?;
                (status, Some(gas_used), logs)
            }
            None => (TxStatus::Pending, None, Vec::new()),
        };

        Ok(Some(Transaction {
            chain,
            hash: tx_hash,
            status,
            block_number,
            block_hash,
            tx_index,
            from,
            to,
            value,
            gas_price,
            gas_used,
            gas_limit,
            nonce,
            tx_type,
            input,
            logs,
            raw_json,
        }))
    }
}

fn raw_log_to_entry(raw: RawLog) -> Result<LogEntry, DomainError> {
    let address = Address::from_hex(&raw.address)?;
    let topics = raw
        .topics
        .iter()
        .map(|t| parse_topic(t))
        .collect::<Result<Vec<_>, _>>()?;
    let data = parse_hex_bytes(&raw.data)?;
    Ok(LogEntry {
        address,
        topics,
        data,
    })
}

fn parse_topic(s: &str) -> Result<[u8; 32], DomainError> {
    let stripped = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .ok_or_else(|| DomainError::InvalidInput(format!("topic must start with 0x: {s}")))?;
    if stripped.len() != 64 {
        return Err(DomainError::InvalidInput(format!(
            "topic must be 32 bytes / 64 hex chars, got {}",
            stripped.len()
        )));
    }
    let mut bytes = [0u8; 32];
    hex::decode_to_slice(stripped, &mut bytes)
        .map_err(|e| DomainError::InvalidInput(format!("invalid hex: {e}")))?;
    Ok(bytes)
}

fn opt_block_number(s: Option<&str>) -> Result<Option<BlockNumber>, DomainError> {
    match s {
        Some(hex) if !hex.is_empty() => {
            let n = parse_hex_u64(hex).map_err(|e| e.into_domain())?;
            Ok(Some(BlockNumber::new(n)))
        }
        _ => Ok(None),
    }
}

fn opt_block_hash(s: Option<&str>) -> Result<Option<BlockHash>, DomainError> {
    match s {
        Some(hex) if !hex.is_empty() && hex != "0x" => Ok(Some(BlockHash::from_hex(hex)?)),
        _ => Ok(None),
    }
}

fn opt_hex_u64(s: Option<&str>) -> Result<Option<u64>, DomainError> {
    match s {
        Some(hex) if !hex.is_empty() => {
            Ok(Some(parse_hex_u64(hex).map_err(|e| e.into_domain())?))
        }
        _ => Ok(None),
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
    let owned;
    let input: &str = if stripped.len().is_multiple_of(2) {
        stripped
    } else {
        owned = format!("0{stripped}");
        owned.as_str()
    };
    hex::decode(input).map_err(|e| DomainError::InvalidInput(format!("invalid hex: {e}")))
}
