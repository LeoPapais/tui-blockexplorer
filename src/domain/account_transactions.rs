//! Normal account transactions from an external indexer (Etherscan `txlist`).
//!
//! See `plan/18-shell-navigation-and-feeds.md` Slice D.

use crate::domain::{Address, BlockNumber, Chain, TxHash, Wei};

/// One outward normal transaction row from `module=account&action=txlist`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountTx {
    pub chain: Chain,
    pub block_number: BlockNumber,
    pub tx_hash: TxHash,
    pub from: Address,
    /// `None` for contract-creation transactions.
    pub to: Option<Address>,
    pub value: Wei,
}

/// Opaque pagination cursor: next Etherscan `page` (1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountTxCursor(pub u32);

/// One page of account transactions (newest first).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountTxPage {
    pub txs: Vec<AccountTx>,
    pub next_cursor: Option<AccountTxCursor>,
}
