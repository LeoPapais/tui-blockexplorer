//! Alchemy-backed [`TxTracePort`] adapter.
//!
//! Wraps `trace_replayTransaction` with `["stateDiff"]` and translates
//! the Parity-style state-diff payload into the domain
//! [`StateDiff`] tree.
//!
//! See `plan/4-tx-detail.md` section 12.4.3.

use serde::Deserialize;
use serde_json::{Value, json};

use super::client::{RpcClient, RpcError};
use crate::{
    application::ports::TxTracePort,
    domain::{
        Address, AddressStateDiff, Chain, DiffChange, DomainError, StateDiff, StorageSlotDiff,
        TxHash,
    },
};

#[derive(Debug, Deserialize)]
struct ReplayResponse {
    #[serde(default, rename = "stateDiff")]
    state_diff: Option<serde_json::Map<String, Value>>,
}

#[derive(Debug, Clone)]
pub struct AlchemyTxTracer {
    client: RpcClient,
}

impl AlchemyTxTracer {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }
}

impl TxTracePort for AlchemyTxTracer {
    async fn state_diff(&self, hash: TxHash, _chain: Chain) -> Result<StateDiff, DomainError> {
        let params = json!([hash.to_hex(), ["stateDiff"]]);
        let response: ReplayResponse = self
            .client
            .call("trace_replayTransaction", params)
            .await
            .map_err(|e: RpcError| match e {
                RpcError::Rpc { code: -32601, .. } => DomainError::FeatureUnavailable,
                other => other.into_domain(),
            })?;

        let Some(map) = response.state_diff else {
            return Ok(StateDiff::default());
        };

        let mut entries = Vec::with_capacity(map.len());
        for (addr_hex, entry) in map {
            let address = Address::from_hex(&addr_hex)?;
            let balance = read_change(entry.get("balance"));
            let nonce = read_change(entry.get("nonce"));
            let code = read_change(entry.get("code"));
            let storage = read_storage(entry.get("storage"))?;
            entries.push(AddressStateDiff {
                address,
                balance,
                nonce,
                code,
                storage,
            });
        }
        Ok(StateDiff { entries })
    }
}

/// Parity-style diff values come in three shapes:
///   "="                           unchanged
///   {"+": "0x.."}                 added
///   {"-": "0x.."}                 removed
///   {"*": {"from": "0x..", "to": "0x.."}}   changed
fn read_change(value: Option<&Value>) -> DiffChange {
    let Some(value) = value else {
        return DiffChange::Unchanged;
    };
    if value.as_str() == Some("=") {
        return DiffChange::Unchanged;
    }
    if let Some(obj) = value.as_object() {
        if let Some(v) = obj.get("+").and_then(Value::as_str) {
            return DiffChange::Added(v.to_string());
        }
        if let Some(v) = obj.get("-").and_then(Value::as_str) {
            return DiffChange::Removed(v.to_string());
        }
        if let Some(inner) = obj.get("*").and_then(Value::as_object) {
            let from = inner
                .get("from")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let to = inner
                .get("to")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            return DiffChange::Changed { from, to };
        }
    }
    DiffChange::Unchanged
}

fn read_storage(value: Option<&Value>) -> Result<Vec<StorageSlotDiff>, DomainError> {
    let Some(obj) = value.and_then(Value::as_object) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(obj.len());
    for (slot, change) in obj {
        let change = read_change(Some(change));
        if change.is_change() {
            out.push(StorageSlotDiff {
                slot: slot.clone(),
                change,
            });
        }
    }
    Ok(out)
}
