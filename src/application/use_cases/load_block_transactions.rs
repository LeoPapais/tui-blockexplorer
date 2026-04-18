//! Use case: load one page of transactions for the Block Detail
//! Transactions tab.
//!
//! Thin orchestrator over [`BlockReceiptsPort`]: fetches the full
//! list of block transactions (categorised per [`TxCategory`] by the
//! adapter), slices it according to [`BlockTxCursor`] and honours a
//! [`CancelFlag`] so the dispatcher can drop in-flight work when the
//! user switches blocks mid-load.
//!
//! See `plan/3-block-detail.md` §12.3.

use crate::{
    application::{CancelFlag, ports::BlockReceiptsPort},
    domain::{BlockId, BlockTxCursor, BlockTxPage, Chain, DomainError},
};

/// Run the use case.
///
/// - `cancel_before` short-circuits the network call when the flag
///   is already set, returning `Ok(None)`.
/// - `cancel_after_fetch` short-circuits after the fetch but before
///   the slice, mirroring what happens when the user switches
///   blocks while the request is in flight.
///
/// A successful call returns `Ok(Some(page))`. Provider failures
/// bubble up as `Err(_)`.
pub async fn run<P: BlockReceiptsPort>(
    port: &P,
    id: BlockId,
    chain: Chain,
    cursor: BlockTxCursor,
    cancel: &CancelFlag,
) -> Result<Option<BlockTxPage>, DomainError> {
    if cancel.is_cancelled() {
        return Ok(None);
    }
    let rows = port.get_transactions(id, chain).await?;
    if cancel.is_cancelled() {
        return Ok(None);
    }
    Ok(Some(paginate(rows, cursor)))
}

/// Slice a full `Vec<BlockTxReceipt>` into a `BlockTxPage` according
/// to `cursor`. Exposed for unit testing; callers go through `run`.
#[must_use]
pub fn paginate(rows: Vec<crate::domain::BlockTxReceipt>, cursor: BlockTxCursor) -> BlockTxPage {
    let total = rows.len();
    if cursor.offset >= total || cursor.page_size == 0 {
        return BlockTxPage {
            rows: Vec::new(),
            next: None,
            total,
        };
    }
    let end = cursor.offset.saturating_add(cursor.page_size).min(total);
    let page: Vec<_> = rows.into_iter().skip(cursor.offset).take(end - cursor.offset).collect();
    let next = if end < total { Some(cursor.next()) } else { None };
    BlockTxPage {
        rows: page,
        next,
        total,
    }
}
