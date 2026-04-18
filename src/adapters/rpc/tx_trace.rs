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
        Address, AddressStateDiff, CallKind, CallNode, Chain, DiffChange, DomainError, StateDiff,
        StorageSlotDiff, TxHash, Wei,
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

    /// Fetch the transaction's call tree, preferring the Parity-style
    /// `trace_transaction` (flat array + `trace_address` used to
    /// rebuild the tree) and falling back to Geth's
    /// `debug_traceTransaction` with `{"tracer": "callTracer"}`,
    /// which already returns a tree directly.
    ///
    /// See `plan/4-tx-detail.md` section 12.6.5.
    async fn call_tree(&self, hash: TxHash, _chain: Chain) -> Result<CallNode, DomainError> {
        // Try the Parity namespace first.
        match self
            .client
            .call::<_, Vec<Value>>("trace_transaction", json!([hash.to_hex()]))
            .await
        {
            Ok(flat) => {
                return parity_flat_to_tree(flat);
            }
            Err(RpcError::Rpc { code: -32601, .. }) => {
                // Fall through to debug_traceTransaction.
            }
            Err(other) => return Err(other.into_domain()),
        }

        // debug_traceTransaction callTracer fallback.
        match self
            .client
            .call::<_, Value>(
                "debug_traceTransaction",
                json!([hash.to_hex(), {"tracer": "callTracer"}]),
            )
            .await
        {
            Ok(v) => debug_calltracer_to_tree(&v),
            Err(RpcError::Rpc { code: -32601, .. }) => Err(DomainError::FeatureUnavailable),
            Err(other) => Err(other.into_domain()),
        }
    }
}

/// Map a Parity `trace_transaction` flat array into a tree by
/// consuming each frame's `traceAddress` (the zero-based path into
/// the tree).
fn parity_flat_to_tree(frames: Vec<Value>) -> Result<CallNode, DomainError> {
    if frames.is_empty() {
        return Err(DomainError::FeatureUnavailable);
    }
    // Sort frames by trace_address length ascending so parents are
    // processed before children.
    let mut with_path: Vec<(Vec<usize>, Value)> = frames
        .into_iter()
        .map(|f| {
            let path = f
                .get("traceAddress")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_u64().map(|n| n as usize))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (path, f)
        })
        .collect();
    with_path.sort_by_key(|(p, _)| p.len());

    let (_, root_frame) = with_path.remove(0);
    let mut root = parity_frame_to_node(&root_frame)?;
    for (path, frame) in with_path {
        let node = parity_frame_to_node(&frame)?;
        insert_into_tree(&mut root, &path, node)?;
    }
    Ok(root)
}

fn insert_into_tree(
    root: &mut CallNode,
    path: &[usize],
    child: CallNode,
) -> Result<(), DomainError> {
    if path.is_empty() {
        return Err(DomainError::Internal(
            "parity trace: empty path on non-root frame".into(),
        ));
    }
    let mut cursor = root;
    for (i, idx) in path.iter().enumerate() {
        if i + 1 == path.len() {
            cursor.children.push(child);
            return Ok(());
        }
        cursor = cursor
            .children
            .get_mut(*idx)
            .ok_or_else(|| DomainError::Internal("parity trace: gap in traceAddress".into()))?;
    }
    Ok(())
}

fn parity_frame_to_node(frame: &Value) -> Result<CallNode, DomainError> {
    let kind = frame
        .get("type")
        .and_then(Value::as_str)
        .map(CallKind::from_str_upper)
        .unwrap_or(CallKind::Unknown);
    let action = frame.get("action").cloned().unwrap_or(Value::Null);
    let result = frame.get("result").cloned().unwrap_or(Value::Null);
    let error = frame
        .get("error")
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let from = action
        .get("from")
        .and_then(Value::as_str)
        .map(Address::from_hex)
        .transpose()?
        .unwrap_or_else(|| Address::from_bytes([0u8; 20]));
    let to = action
        .get("to")
        .and_then(Value::as_str)
        .map(Address::from_hex)
        .transpose()?
        .or_else(|| {
            result
                .get("address")
                .and_then(Value::as_str)
                .and_then(|s| Address::from_hex(s).ok())
        });
    let value = action
        .get("value")
        .and_then(Value::as_str)
        .map(parse_hex_u128)
        .transpose()?
        .unwrap_or(0);
    let input = action
        .get("input")
        .or_else(|| action.get("init"))
        .and_then(Value::as_str)
        .map(decode_hex_bytes)
        .transpose()?
        .unwrap_or_default();
    let output = result
        .get("output")
        .or_else(|| result.get("code"))
        .and_then(Value::as_str)
        .map(decode_hex_bytes)
        .transpose()?
        .unwrap_or_default();
    let gas_used = result
        .get("gasUsed")
        .and_then(Value::as_str)
        .map(parse_hex_u64)
        .transpose()?
        .unwrap_or(0);
    Ok(CallNode {
        kind,
        from,
        to,
        value: Wei::new(value),
        input,
        output,
        gas_used,
        error,
        children: Vec::new(),
    })
}

fn debug_calltracer_to_tree(v: &Value) -> Result<CallNode, DomainError> {
    let kind = v
        .get("type")
        .and_then(Value::as_str)
        .map(CallKind::from_str_upper)
        .unwrap_or(CallKind::Unknown);
    let from = v
        .get("from")
        .and_then(Value::as_str)
        .map(Address::from_hex)
        .transpose()?
        .unwrap_or_else(|| Address::from_bytes([0u8; 20]));
    let to = v
        .get("to")
        .and_then(Value::as_str)
        .map(Address::from_hex)
        .transpose()?;
    let value = v
        .get("value")
        .and_then(Value::as_str)
        .map(parse_hex_u128)
        .transpose()?
        .unwrap_or(0);
    let input = v
        .get("input")
        .and_then(Value::as_str)
        .map(decode_hex_bytes)
        .transpose()?
        .unwrap_or_default();
    let output = v
        .get("output")
        .and_then(Value::as_str)
        .map(decode_hex_bytes)
        .transpose()?
        .unwrap_or_default();
    let gas_used = v
        .get("gasUsed")
        .and_then(Value::as_str)
        .map(parse_hex_u64)
        .transpose()?
        .unwrap_or(0);
    let error = v
        .get("error")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let children = v
        .get("calls")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(debug_calltracer_to_tree)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(CallNode {
        kind,
        from,
        to,
        value: Wei::new(value),
        input,
        output,
        gas_used,
        error,
        children,
    })
}

fn parse_hex_u128(s: &str) -> Result<u128, DomainError> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    if stripped.is_empty() {
        return Ok(0);
    }
    u128::from_str_radix(stripped, 16)
        .map_err(|e| DomainError::Internal(format!("invalid hex number: {e}")))
}

fn parse_hex_u64(s: &str) -> Result<u64, DomainError> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    if stripped.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(stripped, 16)
        .map_err(|e| DomainError::Internal(format!("invalid hex number: {e}")))
}

fn decode_hex_bytes(s: &str) -> Result<Vec<u8>, DomainError> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    if stripped.is_empty() {
        return Ok(Vec::new());
    }
    hex::decode(stripped).map_err(|e| DomainError::Internal(format!("invalid hex bytes: {e}")))
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
