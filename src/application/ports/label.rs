//! Outbound port for turning a raw address into a human-readable
//! [`Label`].
//!
//! Used first by the Block Detail Overview tab to annotate the
//! miner / fee-recipient row (and the Polygon signer row once the
//! Bor seal-hash RLP lands — see `plan/15-backlog.md §3.5`), and
//! later by the Address / Token screens.
//!
//! See `plan/3-block-detail.md` §12.4.

use crate::domain::{Address, Chain, DomainError, Label};

pub trait LabelPort: Send + Sync {
    /// Look up the label for `address` on `chain`. Returns
    /// `Ok(None)` when nothing is known; provider failures bubble
    /// up as `Err(_)`.
    fn label_for(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<Label>, DomainError>> + Send;
}
