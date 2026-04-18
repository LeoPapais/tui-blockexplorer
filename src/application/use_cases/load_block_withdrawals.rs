//! Use case: surface post-Shanghai withdrawals for the Block Detail
//! "Blobs / Withdrawals" tab.
//!
//! Pure passthrough over the already-loaded [`Block`] entity: the
//! `withdrawals` array is populated by the block reader directly
//! from the RPC response, so this use case only owns framing (paging
//! today is trivial — the UI always renders the full list; once a
//! cursor lands it can be threaded through here).
//!
//! Beacon `blob_sidecars` remains deferred (see
//! `plan/3-block-detail.md` §13). The Blobs half of the tab renders
//! a documented placeholder until a `BeaconApiPort` ships.
//!
//! See `plan/3-block-detail.md` §12.5.

use crate::domain::{Block, Withdrawal};

/// Return the withdrawals attached to a block. Returns an empty
/// slice on pre-Shanghai blocks and chains that do not implement
/// EIP-4895.
#[must_use]
pub fn run(block: &Block) -> Vec<Withdrawal> {
    block.withdrawals.clone()
}
