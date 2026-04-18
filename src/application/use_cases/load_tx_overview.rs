//! Use case: load the full Transaction entity shown on the Overview
//! tab of the TxDetail screen, enriched with decoded method and logs.
//!
//! Signature decoding cascade (plan/4 §12.4.2 + plan/15 §3.3):
//!   direct ABI
//!     -> implementation ABI (via EIP-1967 proxy detection)
//!     -> signature directory (openchain/4byte-compatible)
//!     -> raw selector / topic fallback.
//!
//! See `plan/4-tx-detail.md` sections 12.1 and 12.4.2 and
//! `plan/15-backlog.md` section 3.3.

use serde_json::Value;

use crate::{
    application::{
        DecodedLog, DecodedMethod, DecodedSignature, LoadStatus, SignatureSource, TxView,
        ports::{
            ContractSourcePort, ProxyDetectionPort, SignatureDirectoryPort, TxReaderPort,
            TxSimulationPort, TxTracePort,
        },
    },
    domain::{Address, Chain, DomainError, LogEntry, TxHash},
};

pub async fn run<R: TxReaderPort>(
    reader: &R,
    hash: TxHash,
    chain: Chain,
) -> Result<TxView, DomainError> {
    match reader.get(hash, chain).await? {
        Some(tx) => Ok(TxView::bare(tx)),
        None => Err(DomainError::NotFound),
    }
}

/// Enriched variant that additionally resolves method / event
/// signatures. Missing adapters / lookups degrade to best-effort
/// rather than failing the call.
pub async fn run_with_decoding<R, C, S, P>(
    reader: &R,
    contract_source: &C,
    signatures: &S,
    proxy_detector: &P,
    hash: TxHash,
    chain: Chain,
) -> Result<TxView, DomainError>
where
    R: TxReaderPort,
    C: ContractSourcePort,
    S: SignatureDirectoryPort,
    P: ProxyDetectionPort,
{
    let tx = match reader.get(hash, chain).await? {
        Some(tx) => tx,
        None => return Err(DomainError::NotFound),
    };

    // --- Method signature decoding --------------------------------
    let decoded_method = match tx.selector() {
        Some(selector) => {
            decode_method(
                contract_source,
                signatures,
                proxy_detector,
                &tx,
                selector,
                chain,
            )
            .await
        }
        None => None,
    };

    // --- Receipt-log decoding -------------------------------------
    let mut decoded_logs = Vec::with_capacity(tx.logs.len());
    for raw in tx.logs.iter() {
        let sig = decode_log(contract_source, signatures, proxy_detector, raw, chain).await;
        decoded_logs.push(DecodedLog {
            raw: raw.clone(),
            signature: sig,
        });
    }

    Ok(TxView {
        tx,
        decoded_method,
        decoded_logs,
        asset_changes: LoadStatus::Pending,
        state_diff: LoadStatus::Pending,
    })
}

/// Resolve asset changes for a loaded [`TxView`]. Uses
/// [`LoadStatus`] so the UI can distinguish between "not available
/// on this chain" and a transient fetch failure.
pub async fn load_asset_changes<S: TxSimulationPort>(sim: &S, view: &mut TxView, chain: Chain) {
    let status = match sim.simulate_asset_changes(&view.tx, chain).await {
        Ok(changes) => LoadStatus::Loaded(changes),
        Err(DomainError::FeatureUnavailable) => LoadStatus::Unsupported,
        Err(err) => LoadStatus::Failed(format!("{err}")),
    };
    view.asset_changes = status;
}

/// Resolve the state diff for a loaded [`TxView`].
pub async fn load_state_diff<T: TxTracePort>(tracer: &T, view: &mut TxView, chain: Chain) {
    let status = match tracer.state_diff(view.tx.hash, chain).await {
        Ok(diff) => LoadStatus::Loaded(diff),
        Err(DomainError::FeatureUnavailable) => LoadStatus::Unsupported,
        Err(err) => LoadStatus::Failed(format!("{err}")),
    };
    view.state_diff = status;
}

/// Walk the decoding cascade for a method selector. Tried in order:
///
/// 1. Direct ABI of `tx.to`.
/// 2. ABI of the implementation behind an EIP-1967 proxy at `tx.to`.
/// 3. Signature directory.
///
/// Errors from individual adapters are swallowed intentionally: a
/// missing ETHERSCAN_API_KEY must not make the method row disappear
/// when the signature directory would otherwise resolve it.
async fn decode_method<C, S, P>(
    contract_source: &C,
    signatures: &S,
    proxy_detector: &P,
    tx: &crate::domain::Transaction,
    selector: [u8; 4],
    chain: Chain,
) -> Option<DecodedMethod>
where
    C: ContractSourcePort,
    S: SignatureDirectoryPort,
    P: ProxyDetectionPort,
{
    if let Some(to) = tx.to {
        // Direct ABI.
        if let Ok(Some(abi)) = contract_source.get_abi(to, chain).await
            && let Some(signature) = match_selector_in_abi(&abi.abi, selector)
        {
            return Some(DecodedMethod {
                signature,
                source: SignatureSource::Abi,
            });
        }
        // Proxy implementation ABI.
        if let Some((implementation, abi)) =
            proxy_implementation_abi(contract_source, proxy_detector, to, chain).await
            && let Some(signature) = match_selector_in_abi(&abi, selector)
        {
            return Some(DecodedMethod {
                signature,
                source: SignatureSource::ProxyAbi {
                    proxy: to,
                    implementation,
                },
            });
        }
    }
    if let Ok(Some(signature)) = signatures.lookup_selector(selector).await {
        return Some(DecodedMethod {
            signature,
            source: SignatureSource::SignatureDirectory,
        });
    }
    None
}

async fn decode_log<C, S, P>(
    contract_source: &C,
    signatures: &S,
    proxy_detector: &P,
    log: &LogEntry,
    chain: Chain,
) -> Option<DecodedSignature>
where
    C: ContractSourcePort,
    S: SignatureDirectoryPort,
    P: ProxyDetectionPort,
{
    let topic0 = log.topics.first().copied()?;
    // Direct ABI.
    if let Ok(Some(abi)) = contract_source.get_abi(log.address, chain).await
        && let Some(signature) = match_event_topic_in_abi(&abi.abi, topic0)
    {
        return Some(DecodedSignature {
            signature,
            source: SignatureSource::Abi,
        });
    }
    // Proxy implementation ABI. Events for proxy-backed ERC-20s are
    // emitted by the proxy but declared on the implementation, so the
    // same fallback applies to topics.
    if let Some((implementation, abi)) =
        proxy_implementation_abi(contract_source, proxy_detector, log.address, chain).await
        && let Some(signature) = match_event_topic_in_abi(&abi, topic0)
    {
        return Some(DecodedSignature {
            signature,
            source: SignatureSource::ProxyAbi {
                proxy: log.address,
                implementation,
            },
        });
    }
    if let Ok(Some(signature)) = signatures.lookup_event_topic(topic0).await {
        return Some(DecodedSignature {
            signature,
            source: SignatureSource::SignatureDirectory,
        });
    }
    None
}

/// Resolve the implementation address behind `proxy` and fetch its
/// ABI. Returns `None` when the adapter has nothing to offer, either
/// because detection misses or the implementation has no verified
/// ABI.
async fn proxy_implementation_abi<C, P>(
    contract_source: &C,
    proxy_detector: &P,
    proxy: Address,
    chain: Chain,
) -> Option<(Address, String)>
where
    C: ContractSourcePort,
    P: ProxyDetectionPort,
{
    let info = proxy_detector.detect(proxy, chain).await.ok().flatten()?;
    let abi = contract_source
        .get_abi(info.implementation, chain)
        .await
        .ok()
        .flatten()?;
    Some((info.implementation, abi.abi))
}

// ---------------------------------------------------------------------------
// Minimal ABI matchers
// ---------------------------------------------------------------------------
//
// The full ABI decoder lives elsewhere. We only need to map a 4-byte
// selector / event topic hash to the canonical signature string; the
// adapter could pull in a heavy-duty crate to parse the ABI JSON, but
// for the MVP we do a lightweight traversal that does not introduce
// a new dependency. When actual argument decoding lands we will
// revisit this and likely reach for `alloy-json-abi`.

fn match_selector_in_abi(abi_json: &str, selector: [u8; 4]) -> Option<String> {
    let abi: Value = serde_json::from_str(abi_json).ok()?;
    let entries = abi.as_array()?;
    for entry in entries {
        if entry.get("type").and_then(Value::as_str) != Some("function") {
            continue;
        }
        let Some(signature) = build_signature(entry) else {
            continue;
        };
        let computed = crate::domain::contract_source::selector_for(&signature);
        if computed == selector {
            return Some(signature);
        }
    }
    None
}

fn match_event_topic_in_abi(abi_json: &str, topic: [u8; 32]) -> Option<String> {
    let abi: Value = serde_json::from_str(abi_json).ok()?;
    let entries = abi.as_array()?;
    for entry in entries {
        if entry.get("type").and_then(Value::as_str) != Some("event") {
            continue;
        }
        let Some(signature) = build_signature(entry) else {
            continue;
        };
        let computed = crate::domain::contract_source::event_topic_for(&signature);
        if computed == topic {
            return Some(signature);
        }
    }
    None
}

fn build_signature(entry: &Value) -> Option<String> {
    let name = entry.get("name").and_then(Value::as_str)?;
    let inputs = entry.get("inputs").and_then(Value::as_array)?;
    let mut parts = Vec::with_capacity(inputs.len());
    for input in inputs {
        let ty = input.get("type").and_then(Value::as_str)?;
        parts.push(ty.to_string());
    }
    Some(format!("{name}({})", parts.join(",")))
}
