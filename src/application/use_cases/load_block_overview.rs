//! Use case: load the full Block entity shown on the Overview tab of
//! the Block Detail screen.
//!
//! Missing blocks surface as `DomainError::NotFound` so the UI does
//! not have to null-check downstream. See `plan/3-block-detail.md`
//! section 11.1.

use crate::{
    application::ports::BlockReaderPort,
    domain::{Block, BlockId, Chain, DomainError},
};

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
