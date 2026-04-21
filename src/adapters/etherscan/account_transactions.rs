//! Etherscan V2 [`AccountTransactionsPort`]: `account` / `txlist`.
//!
//! Endpoint: GET `https://api.etherscan.io/v2/api` with
//! `module=account`, `action=txlist`, `address`, `page`, `offset`,
//! `sort=desc`, `chainid`, `apikey`.
//!
//! See `plan/18-shell-navigation-and-feeds.md` Slice D.

use serde::Deserialize;

use super::client::{EtherscanClient, EtherscanError};
use crate::{
    application::ports::AccountTransactionsPort,
    domain::{
        AccountTx, AccountTxCursor, AccountTxPage, Address, BlockNumber, Chain, DomainError,
        TxHash, Wei,
    },
};

/// Page size for `txlist` (`offset` parameter).
const TXLIST_OFFSET: u32 = 20;

#[derive(Debug, Clone)]
pub struct EtherscanAccountTransactions {
    client: EtherscanClient,
}

impl EtherscanAccountTransactions {
    #[must_use]
    pub fn new(client: EtherscanClient) -> Self {
        Self { client }
    }
}

#[derive(Debug, Deserialize)]
struct GenericResponse {
    status: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    result: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct RawTxRow {
    #[serde(rename = "blockNumber")]
    block_number: String,
    hash: String,
    from: String,
    #[serde(default)]
    to: String,
    value: String,
}

impl AccountTransactionsPort for EtherscanAccountTransactions {
    async fn list_for_address(
        &self,
        address: Address,
        chain: Chain,
        cursor: Option<AccountTxCursor>,
    ) -> Result<AccountTxPage, DomainError> {
        let page = cursor.map(|c| c.0).unwrap_or(1);
        let addr_hex = address.to_hex();
        let page_str = page.to_string();
        let offset_str = TXLIST_OFFSET.to_string();
        let response = self
            .client
            .get(
                chain,
                "account",
                "txlist",
                &[
                    ("address", addr_hex.as_str()),
                    ("page", page_str.as_str()),
                    ("offset", offset_str.as_str()),
                    ("sort", "desc"),
                ],
            )
            .await
            .map_err(EtherscanError::into_domain)?;

        parse_txlist_response(chain, page, response)
    }
}

fn parse_txlist_response(
    chain: Chain,
    requested_page: u32,
    body: serde_json::Value,
) -> Result<AccountTxPage, DomainError> {
    let parsed: GenericResponse = serde_json::from_value(body)
        .map_err(|e| DomainError::Internal(format!("etherscan txlist decode: {e}")))?;

    if parsed.result.is_array() {
        let arr = parsed.result.as_array().expect("checked is_array");
        let mut txs = Vec::with_capacity(arr.len());
        for row in arr {
            let raw: RawTxRow = serde_json::from_value(row.clone())
                .map_err(|e| DomainError::Internal(format!("etherscan txlist row decode: {e}")))?;
            txs.push(map_row(chain, raw)?);
        }
        let next_cursor = if txs.len() == TXLIST_OFFSET as usize {
            Some(AccountTxCursor(requested_page.saturating_add(1)))
        } else {
            None
        };
        return Ok(AccountTxPage { txs, next_cursor });
    }

    if parsed.status == "1" {
        return Err(DomainError::Internal(
            "etherscan txlist: unexpected non-array result".into(),
        ));
    }

    // Empty / not found: often `status=0`, `message` like "No transactions found", `result=[]`.
    if parsed.result.is_string() {
        let s = parsed.result.as_str().expect("checked is_string");
        if s.is_empty() {
            return Ok(AccountTxPage::default());
        }
        return Err(DomainError::Internal(format!("etherscan txlist: {s}")));
    }

    Err(DomainError::Internal(format!(
        "etherscan txlist: {}",
        parsed.message
    )))
}

/// When no Etherscan API key is configured at the composition root,
/// account transactions stay empty without failing the address feed.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopAccountTransactions;

impl AccountTransactionsPort for NoopAccountTransactions {
    async fn list_for_address(
        &self,
        _address: Address,
        _chain: Chain,
        _cursor: Option<AccountTxCursor>,
    ) -> Result<AccountTxPage, DomainError> {
        Ok(AccountTxPage::default())
    }
}

/// Live Etherscan vs noop, for a single [`AccountTransactionsPort`] binding
/// at the composition root.
#[derive(Debug, Clone)]
pub enum AccountTransactionsAdapter {
    Etherscan(EtherscanAccountTransactions),
    Noop(NoopAccountTransactions),
}

impl AccountTransactionsAdapter {
    #[must_use]
    pub fn from_optional_key(api_key: Option<String>) -> Self {
        match api_key.and_then(|k| EtherscanClient::with_default_http(k).ok()) {
            Some(client) => Self::Etherscan(EtherscanAccountTransactions::new(client)),
            None => Self::Noop(NoopAccountTransactions),
        }
    }
}

impl AccountTransactionsPort for AccountTransactionsAdapter {
    async fn list_for_address(
        &self,
        address: Address,
        chain: Chain,
        cursor: Option<AccountTxCursor>,
    ) -> Result<AccountTxPage, DomainError> {
        match self {
            Self::Etherscan(inner) => inner.list_for_address(address, chain, cursor).await,
            Self::Noop(inner) => inner.list_for_address(address, chain, cursor).await,
        }
    }
}

fn map_row(chain: Chain, raw: RawTxRow) -> Result<AccountTx, DomainError> {
    let block_u64: u64 = raw
        .block_number
        .parse()
        .map_err(|_| DomainError::Internal("invalid blockNumber in txlist".into()))?;
    let tx_hash = TxHash::from_hex(&raw.hash)?;
    let from = Address::from_hex(&raw.from)?;
    let to = if raw.to.is_empty() {
        None
    } else {
        Some(Address::from_hex(&raw.to)?)
    };
    let value_u128: u128 = raw
        .value
        .parse()
        .map_err(|_| DomainError::Internal("invalid value in txlist".into()))?;
    Ok(AccountTx {
        chain,
        block_number: BlockNumber::new(block_u64),
        tx_hash,
        from,
        to,
        value: Wei::new(value_u128),
    })
}
