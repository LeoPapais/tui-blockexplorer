//! Outbound port for normal transactions indexed for an address.
//!
//! Backed by Etherscan V2 `module=account&action=txlist` in live mode.
//!
//! See `plan/18-shell-navigation-and-feeds.md` Slice D (`AccountTransactionsPort`).

use crate::domain::{AccountTxCursor, AccountTxPage, Address, Chain, DomainError};

pub trait AccountTransactionsPort: Send + Sync {
    /// Fetch one page of normal transactions where `from` or `to` is `address`.
    /// `cursor` is `None` for the first page, or the `next_cursor` from a prior page.
    fn list_for_address(
        &self,
        address: Address,
        chain: Chain,
        cursor: Option<AccountTxCursor>,
    ) -> impl std::future::Future<Output = Result<AccountTxPage, DomainError>> + Send;
}
