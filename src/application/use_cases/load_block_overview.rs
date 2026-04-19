//! Use case: load the full Block entity shown on the Overview tab of
//! the Block Detail screen plus the human labels associated with the
//! miner and the Polygon extraData signer.
//!
//! Missing blocks surface as `DomainError::NotFound` so the UI does
//! not have to null-check downstream. See `plan/3-block-detail.md`
//! §11.1 and §12.4.

use crate::{
    application::ports::{BlockReaderPort, LabelPort},
    domain::{Address, Block, BlockId, Chain, DomainError, Label},
};

/// View-model produced by `load_block_overview::run_with_labels`.
/// Wraps the [`Block`] entity with optional human labels for the
/// miner and the Polygon signer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockOverview {
    pub block: Block,
    pub miner_label: Option<Label>,
    /// Label for `block.extra_signer` when that field is set;
    /// `None` on chains where the miner is already the real author
    /// or when the signer has no registered label.
    pub signer_label: Option<Label>,
}

/// Backwards-compatible helper retained for call sites that only
/// need the `Block` entity (no labels needed).
///
/// New callers should prefer [`run_with_labels`] so the Overview
/// tab can render labelled rows.
pub async fn run<P: BlockReaderPort>(
    reader: &P,
    id: BlockId,
    chain: Chain,
) -> Result<Block, DomainError> {
    match reader.get(id, chain).await? {
        Some(block) => Ok(block),
        None => Err(DomainError::NotFound),
    }
}

/// Load the block and look up the miner / signer labels in parallel
/// via the provided [`LabelPort`]. Label-port failures are
/// non-fatal: they degrade to `None`, not to an error, so the
/// Overview tab always renders the block.
pub async fn run_with_labels<R, L>(
    reader: &R,
    labels: &L,
    id: BlockId,
    chain: Chain,
) -> Result<BlockOverview, DomainError>
where
    R: BlockReaderPort,
    L: LabelPort,
{
    let Some(block) = reader.get(id, chain).await? else {
        return Err(DomainError::NotFound);
    };

    let miner_label = soft_label(labels, block.miner, chain).await;
    let signer_label = match block.extra_signer {
        Some(signer) => soft_label(labels, signer, chain).await,
        None => None,
    };

    Ok(BlockOverview {
        block,
        miner_label,
        signer_label,
    })
}

async fn soft_label<L: LabelPort>(labels: &L, address: Address, chain: Chain) -> Option<Label> {
    // Treat provider errors as "no label" so a broken Etherscan
    // key never breaks the Overview tab.
    labels.label_for(address, chain).await.ok().flatten()
}
