//! Alchemy-backed [`TransfersPort`] adapter.
//!
//! Issues two parallel `alchemy_getAssetTransfers` calls
//! (`fromAddress` + `toAddress`) and merges the results by block
//! number descending. Internal transfers are only requested on
//! chains that support them (Ethereum mainnet today).
//!
//! See `plan/6-address-detail.md` section 12.4.1.

use serde::Deserialize;
use serde_json::{Value, json};

use super::client::{RpcClient, RpcError, parse_hex_u64};
use crate::{
    application::ports::TransfersPort,
    domain::{
        Address, BlockNumber, Chain, DomainError, NftKind, TransferAsset, TransferCategory,
        TransferCursor, TransferEvent, TransferPage, TxHash, Wei,
    },
};

/// Hard cap on the events returned per direction. 100 keeps the
/// follow-up rendering snappy and still gives a generous history.
const MAX_PER_DIRECTION: u64 = 100;

/// Merged cap after both directions are combined + deduplicated.
const MAX_TOTAL: usize = 150;

#[derive(Debug, Deserialize)]
struct RawResponse {
    #[serde(default)]
    transfers: Vec<RawTransfer>,
    #[serde(default, rename = "pageKey")]
    page_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTransfer {
    #[serde(rename = "blockNum")]
    block_num: String,
    #[serde(default)]
    hash: String,
    #[serde(default)]
    from: String,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    asset: Option<String>,
    category: String,
    #[serde(default, rename = "rawContract")]
    raw_contract: Option<RawContract>,
    #[serde(default, rename = "erc721TokenId")]
    erc721_token_id: Option<String>,
    #[serde(default, rename = "erc1155Metadata")]
    erc1155_metadata: Option<Vec<Erc1155Entry>>,
}

#[derive(Debug, Deserialize)]
struct RawContract {
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    decimal: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Erc1155Entry {
    #[serde(rename = "tokenId")]
    token_id: String,
    #[serde(default)]
    #[allow(dead_code)]
    value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AlchemyTransfers {
    client: RpcClient,
}

impl AlchemyTransfers {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }

    async fn fetch_direction(
        &self,
        address: Address,
        chain: Chain,
        direction: Direction,
        page_key: Option<&str>,
    ) -> Result<RawResponse, DomainError> {
        let mut params = serde_json::Map::new();
        params.insert("fromBlock".into(), json!("0x0"));
        params.insert("toBlock".into(), json!("latest"));
        params.insert("order".into(), json!("desc"));
        params.insert("withMetadata".into(), json!(false));
        params.insert("excludeZeroValue".into(), json!(true));
        params.insert(
            "maxCount".into(),
            json!(format!("0x{:x}", MAX_PER_DIRECTION)),
        );
        params.insert("category".into(), json!(categories_for(chain)));
        match direction {
            Direction::From => {
                params.insert("fromAddress".into(), json!(address.to_hex()));
            }
            Direction::To => {
                params.insert("toAddress".into(), json!(address.to_hex()));
            }
        }
        if let Some(key) = page_key {
            params.insert("pageKey".into(), json!(key));
        }

        self.client
            .call(
                "alchemy_getAssetTransfers",
                json!([Value::Object(params)]),
            )
            .await
            .map_err(|e: RpcError| match e {
                RpcError::Rpc { code: -32601, .. } => DomainError::FeatureUnavailable,
                other => other.into_domain(),
            })
    }
}

impl TransfersPort for AlchemyTransfers {
    async fn get_for_address(
        &self,
        address: Address,
        chain: Chain,
        cursor: Option<TransferCursor>,
    ) -> Result<TransferPage, DomainError> {
        let (from_key, to_key) = split_cursor(cursor.as_ref());

        let (from_res, to_res) = tokio::join!(
            self.fetch_direction(address, chain, Direction::From, from_key.as_deref()),
            self.fetch_direction(address, chain, Direction::To, to_key.as_deref()),
        );

        let mut events = Vec::new();
        let from_raw = from_res?;
        let to_raw = to_res?;
        for raw in from_raw.transfers.iter().chain(to_raw.transfers.iter()) {
            if let Some(event) = map_transfer(raw, chain) {
                events.push(event);
            }
        }

        events.sort_by_key(|e| std::cmp::Reverse(e.block_number.value()));
        events.dedup_by(|a, b| a.tx_hash == b.tx_hash && a.from == b.from && a.to == b.to);
        events.truncate(MAX_TOTAL);

        let next_cursor = join_cursor(from_raw.page_key.as_deref(), to_raw.page_key.as_deref());
        Ok(TransferPage {
            events,
            next_cursor,
        })
    }
}

#[derive(Copy, Clone)]
enum Direction {
    From,
    To,
}

/// Alchemy's `internal` category is only supported on Ethereum mainnet
/// and Polygon mainnet. On every other chain we omit it to avoid the
/// `"invalid params"` error that Alchemy returns otherwise.
fn categories_for(chain: Chain) -> Vec<&'static str> {
    match chain {
        Chain::Ethereum | Chain::Polygon => {
            vec!["external", "internal", "erc20", "erc721", "erc1155"]
        }
        _ => vec!["external", "erc20", "erc721", "erc1155"],
    }
}

fn map_transfer(raw: &RawTransfer, chain: Chain) -> Option<TransferEvent> {
    let block_number = BlockNumber::new(parse_hex_u64(&raw.block_num).ok()?);
    let tx_hash = TxHash::from_hex(&raw.hash).ok()?;
    let from = Address::from_hex(&raw.from).ok()?;
    let to = raw
        .to
        .as_deref()
        .filter(|s| !s.is_empty() && *s != "0x")
        .and_then(|s| Address::from_hex(s).ok());

    let category = match raw.category.as_str() {
        "external" => TransferCategory::External,
        "internal" => TransferCategory::Internal,
        "erc20" => TransferCategory::Erc20,
        "erc721" => TransferCategory::Erc721,
        "erc1155" => TransferCategory::Erc1155,
        _ => return None,
    };

    let raw_value = raw
        .raw_contract
        .as_ref()
        .and_then(|r| r.value.as_deref())
        .and_then(|s| u128::from_str_radix(s.strip_prefix("0x").unwrap_or(s), 16).ok())
        .unwrap_or(0);

    let contract_address = raw
        .raw_contract
        .as_ref()
        .and_then(|r| r.address.as_deref())
        .filter(|s| !s.is_empty() && *s != "0x")
        .and_then(|s| Address::from_hex(s).ok());

    let symbol = raw
        .asset
        .clone()
        .unwrap_or_else(|| category.label().to_string());

    let asset = match category {
        TransferCategory::External | TransferCategory::Internal => TransferAsset::Native { symbol },
        TransferCategory::Erc20 => TransferAsset::Erc20 {
            contract: contract_address?,
            symbol,
            decimals: parse_decimal(raw.raw_contract.as_ref().and_then(|r| r.decimal.as_deref())),
        },
        TransferCategory::Erc721 => TransferAsset::Nft {
            contract: contract_address?,
            kind: NftKind::Erc721,
            token_id: raw.erc721_token_id.clone().unwrap_or_default(),
            symbol,
        },
        TransferCategory::Erc1155 => {
            let first = raw
                .erc1155_metadata
                .as_ref()
                .and_then(|v| v.first())
                .cloned();
            let token_id = first.as_ref().map(|e| e.token_id.clone()).unwrap_or_default();
            TransferAsset::Nft {
                contract: contract_address?,
                kind: NftKind::Erc1155,
                token_id,
                symbol,
            }
        }
    };

    // For NFTs the Alchemy `rawContract.value` is usually "0x0" /
    // empty, which is not meaningful — default to 1 so the UI
    // renders a reasonable "1x #42".
    let value = match asset {
        TransferAsset::Nft { .. } if raw_value == 0 => Wei::new(1),
        _ => Wei::new(raw_value),
    };

    Some(TransferEvent {
        chain,
        block_number,
        tx_hash,
        from,
        to,
        asset,
        value,
        category,
    })
}

fn parse_decimal(s: Option<&str>) -> u8 {
    s.and_then(|s| u8::from_str_radix(s.strip_prefix("0x").unwrap_or(s), 16).ok())
        .unwrap_or(0)
}

fn split_cursor(cursor: Option<&TransferCursor>) -> (Option<String>, Option<String>) {
    let Some(cursor) = cursor else {
        return (None, None);
    };
    let mut it = cursor.0.splitn(2, '|');
    let from = it.next().filter(|s| !s.is_empty()).map(str::to_string);
    let to = it.next().filter(|s| !s.is_empty()).map(str::to_string);
    (from, to)
}

fn join_cursor(from: Option<&str>, to: Option<&str>) -> Option<TransferCursor> {
    if from.is_none() && to.is_none() {
        return None;
    }
    Some(TransferCursor(format!(
        "{}|{}",
        from.unwrap_or(""),
        to.unwrap_or("")
    )))
}
