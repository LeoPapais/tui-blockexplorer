//! Outbound port for fetching the full `Block` entity behind the
//! Block Detail screen.
//!
//! See `plan/3-block-detail.md` section 11.1.

use crate::domain::{Block, BlockId, Chain, DomainError};

pub trait BlockReaderPort: Send + Sync {
    /// Fetch the full block identified by `id`. Returns `Ok(None)`
    /// when the block does not exist on the chain; surfaces
    /// provider-level failures through `Err(DomainError::*)`.
    fn get(
        &self,
        id: BlockId,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<Block>, DomainError>> + Send;
}
