//! Use case: load one page of normal transactions for an address.
//!
//! See `plan/18-shell-navigation-and-feeds.md` Slice D.

use crate::{
    application::ports::AccountTransactionsPort,
    domain::{AccountTxCursor, AccountTxPage, Address, Chain, DomainError},
};

pub async fn run<A: AccountTransactionsPort>(
    account_tx: &A,
    address: Address,
    chain: Chain,
    cursor: Option<AccountTxCursor>,
) -> Result<AccountTxPage, DomainError> {
    account_tx.list_for_address(address, chain, cursor).await
}
