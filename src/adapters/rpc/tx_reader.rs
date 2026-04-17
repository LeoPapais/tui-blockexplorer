//! Alchemy-backed [`TxReaderPort`].
//!
//! Issues `eth_getTransactionByHash` + `eth_getTransactionReceipt` in
//! parallel with `tokio::join!` and merges the two into a full
//! [`Transaction`] entity. The Raw tab backing is populated from the
//! pretty-printed tx JSON body so users can inspect the exact
//! response shape.
//!
//! See `plan/4-tx-detail.md` section 12.2.

use serde::Deserialize;
use serde_json::Value;

use super::client::{RpcClient, RpcError, parse_hex_u128, parse_hex_u64};
use crate::{
    application::ports::TxReaderPort,
    domain::{
        Address, BlockHash, BlockNumber, Chain, DomainError, Transaction, TxHash, TxStatus,
        TxType, Wei,
    },
};

#[derive(Debug, Deserialize)]
struct RawTx {
    hash: String,
    #[serde(rename = "blockNumber")]
    block_number: String,
    #[serde(rename = "blockHash")]
    block_hash: String,
    #[serde(rename = "transactionIndex")]
    tx_index: String,
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

        // Fetch tx + receipt concurrently. The tx call returns a
        // Value (instead of RawTx) so the Raw tab can keep the
        // original JSON without a second serialization round.
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

        let (Some(tx_value), Some(receipt)) = (tx_value, receipt) else {
            return Ok(None);
        };

        let raw_json = serde_json::to_string_pretty(&tx_value)
            .unwrap_or_else(|_| tx_value.to_string());
        let raw_tx: RawTx = serde_json::from_value(tx_value)
            .map_err(|e| DomainError::Internal(format!("tx decode: {e}")))?;

        let tx_hash = TxHash::from_hex(&raw_tx.hash)?;
        let block_number = BlockNumber::new(
            parse_hex_u64(&raw_tx.block_number).map_err(|e| e.into_domain())?,
        );
        let block_hash = BlockHash::from_hex(&raw_tx.block_hash)?;
        let tx_index = parse_hex_u64(&raw_tx.tx_index).map_err(|e| e.into_domain())?;
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

        let gas_used = parse_hex_u64(&receipt.gas_used).map_err(|e| e.into_domain())?;
        let status = if parse_hex_u64(&receipt.status).unwrap_or(0) == 1 {
            TxStatus::Success
        } else {
            TxStatus::Failed {
                reason: receipt.revert_reason,
            }
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
            raw_json,
        }))
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
