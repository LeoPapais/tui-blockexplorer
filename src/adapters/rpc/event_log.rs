//! Alchemy-backed [`EventLogPort`] adapter.
//!
//! Wraps `eth_getLogs` over a bounded block range. Larger ranges
//! would hit node payload limits; the Contract Detail Events tab
//! therefore paginates 5_000 blocks at a time.
//!
//! See `plan/7-contract-detail.md` section 12.4.3.

use serde::Deserialize;
use serde_json::json;

use super::client::{RpcClient, RpcError};
use crate::{
    application::ports::{BlockRange, EventLogPort},
    domain::{Address, Chain, DomainError, LogEntry},
};

#[derive(Debug, Deserialize)]
struct RawLog {
    address: String,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    data: String,
}

#[derive(Debug, Clone)]
pub struct AlchemyEventLog {
    client: RpcClient,
}

impl AlchemyEventLog {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }
}

impl EventLogPort for AlchemyEventLog {
    async fn get_logs(
        &self,
        address: Address,
        _chain: Chain,
        range: BlockRange,
    ) -> Result<Vec<LogEntry>, DomainError> {
        // Sentinel: `range.to == u64::MAX` means "latest" — we first
        // resolve the head block and then clamp the window to the
        // most recent DEFAULT_WINDOW blocks so providers do not reject
        // the query with "response too large".
        let (from, to) = if range.to.value() == u64::MAX {
            let head_hex: String = self
                .client
                .call("eth_blockNumber", json!([]))
                .await
                .map_err(RpcError::into_domain)?;
            let head = u64::from_str_radix(head_hex.strip_prefix("0x").unwrap_or(&head_hex), 16)
                .map_err(|e| DomainError::Internal(format!("invalid head: {e}")))?;
            let from = head.saturating_sub(DEFAULT_WINDOW.saturating_sub(1));
            (from, head)
        } else {
            (range.from.value(), range.to.value())
        };

        let filter = json!({
            "address": address.to_hex(),
            "fromBlock": format!("0x{:x}", from),
            "toBlock": format!("0x{:x}", to),
        });
        let raw: Vec<RawLog> = self
            .client
            .call("eth_getLogs", json!([filter]))
            .await
            .map_err(|e: RpcError| match e {
                RpcError::Rpc { code: -32601, .. } => DomainError::FeatureUnavailable,
                other => other.into_domain(),
            })?;
        raw.into_iter().map(map_log).collect()
    }
}

/// Default window size (in blocks) used when the caller asks for
/// "latest" logs via the `u64::MAX` sentinel. 5_000 blocks is the
/// conservative limit most providers accept without extra pagination.
const DEFAULT_WINDOW: u64 = 5_000;

fn map_log(raw: RawLog) -> Result<LogEntry, DomainError> {
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
        .ok_or_else(|| DomainError::InvalidInput(format!("topic must start with 0x: {s}")))?;
    if stripped.len() != 64 {
        return Err(DomainError::InvalidInput(format!(
            "topic must be 32 bytes, got {}",
            stripped.len()
        )));
    }
    let mut out = [0u8; 32];
    hex::decode_to_slice(stripped, &mut out)
        .map_err(|e| DomainError::InvalidInput(format!("invalid hex: {e}")))?;
    Ok(out)
}

fn parse_hex_bytes(s: &str) -> Result<Vec<u8>, DomainError> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    if stripped.is_empty() {
        return Ok(Vec::new());
    }
    hex::decode(stripped).map_err(|e| DomainError::InvalidInput(format!("invalid hex: {e}")))
}
