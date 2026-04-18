//! Alchemy-backed [`TxSimulationPort`] adapter.
//!
//! Calls `alchemy_simulateAssetChanges` with a tx object rebuilt from
//! the loaded [`Transaction`] (from / to / value / data / gas).
//!
//! See `plan/4-tx-detail.md` section 12.4.3.

use serde::Deserialize;
use serde_json::{Value, json};

use super::client::{RpcClient, RpcError};
use crate::{
    application::ports::TxSimulationPort,
    domain::{
        Address, AssetChange, AssetChangeKind, AssetKind, Chain, DomainError, Transaction, Wei,
    },
};

#[derive(Debug, Deserialize)]
struct RawSimResponse {
    #[serde(default)]
    changes: Vec<RawAssetChange>,
}

#[derive(Debug, Deserialize)]
struct RawAssetChange {
    #[serde(default, rename = "assetType")]
    asset_type: Option<String>,
    #[serde(default, rename = "changeType")]
    change_type: Option<String>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
    #[serde(default, rename = "rawAmount")]
    raw_amount: Option<String>,
    #[serde(default, rename = "contractAddress")]
    contract_address: Option<String>,
    #[serde(default)]
    symbol: Option<String>,
    #[serde(default)]
    decimals: Option<u8>,
    #[serde(default, rename = "tokenId")]
    token_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AlchemySimulation {
    client: RpcClient,
}

impl AlchemySimulation {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }
}

impl TxSimulationPort for AlchemySimulation {
    async fn simulate_asset_changes(
        &self,
        tx: &Transaction,
        _chain: Chain,
    ) -> Result<Vec<AssetChange>, DomainError> {
        let mut tx_object = serde_json::Map::new();
        tx_object.insert("from".into(), json!(tx.from.to_hex()));
        if let Some(to) = tx.to {
            tx_object.insert("to".into(), json!(to.to_hex()));
        }
        tx_object.insert("value".into(), json!(format!("0x{:x}", tx.value.value())));
        if !tx.input.is_empty() {
            tx_object.insert(
                "data".into(),
                json!(format!("0x{}", hex::encode(&tx.input))),
            );
        }
        tx_object.insert("gas".into(), json!(format!("0x{:x}", tx.gas_limit)));

        let params = json!([Value::Object(tx_object)]);
        let raw: Option<RawSimResponse> = self
            .client
            .call("alchemy_simulateAssetChanges", params)
            .await
            .map_err(|e: RpcError| match e {
                // JSON-RPC: method not found (-32601) means the
                // underlying chain/provider does not support the
                // simulation endpoint. Surface it as a UI-friendly
                // FeatureUnavailable.
                RpcError::Rpc { code: -32601, .. } => DomainError::FeatureUnavailable,
                other => other.into_domain(),
            })?;

        let Some(raw) = raw else {
            return Ok(Vec::new());
        };

        let mut changes = Vec::with_capacity(raw.changes.len());
        for row in raw.changes {
            if let Some(change) = map_change(row)? {
                changes.push(change);
            }
        }
        Ok(changes)
    }
}

fn map_change(raw: RawAssetChange) -> Result<Option<AssetChange>, DomainError> {
    let kind = match raw.change_type.as_deref().map(str::to_ascii_uppercase) {
        Some(s) if s == "TRANSFER" => AssetChangeKind::Transfer,
        Some(s) if s == "APPROVE" => AssetChangeKind::Approve,
        _ => AssetChangeKind::Other,
    };

    let symbol = raw.symbol.clone().unwrap_or_default();
    let decimals = raw.decimals.unwrap_or(0);
    let contract = raw
        .contract_address
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(Address::from_hex)
        .transpose()?;

    let asset = match raw.asset_type.as_deref().map(str::to_ascii_uppercase) {
        Some(s) if s == "NATIVE" => AssetKind::Native,
        Some(s) if s == "ERC20" => {
            let Some(contract) = contract else {
                return Ok(None);
            };
            AssetKind::Erc20 {
                contract,
                symbol,
                decimals,
            }
        }
        Some(s) if s == "ERC721" => {
            let Some(contract) = contract else {
                return Ok(None);
            };
            AssetKind::Erc721 {
                contract,
                symbol,
                token_id: raw.token_id.clone().unwrap_or_default(),
            }
        }
        Some(s) if s == "ERC1155" => {
            let Some(contract) = contract else {
                return Ok(None);
            };
            AssetKind::Erc1155 {
                contract,
                symbol,
                token_id: raw.token_id.clone().unwrap_or_default(),
            }
        }
        _ => return Ok(None),
    };

    let from = raw
        .from
        .as_deref()
        .filter(|s| !s.is_empty() && *s != "0x")
        .map(Address::from_hex)
        .transpose()?;
    let to = raw
        .to
        .as_deref()
        .filter(|s| !s.is_empty() && *s != "0x")
        .map(Address::from_hex)
        .transpose()?;

    let amount = parse_amount(raw.raw_amount.as_deref())?;

    Ok(Some(AssetChange {
        kind,
        asset,
        from,
        to,
        amount,
    }))
}

fn parse_amount(value: Option<&str>) -> Result<Wei, DomainError> {
    match value {
        None | Some("") => Ok(Wei::new(0)),
        Some(s) => {
            let stripped = s.strip_prefix("0x").unwrap_or(s);
            let n = u128::from_str_radix(stripped, 16).or_else(|_| stripped.parse::<u128>());
            Ok(Wei::new(n.unwrap_or(0)))
        }
    }
}
