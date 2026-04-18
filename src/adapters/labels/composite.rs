//! Composite [`LabelPort`] that prefers a primary source and falls
//! back to a secondary when the primary returns `None`.
//!
//! The provenance (well-known vs Etherscan) survives in the returned
//! [`Label::source`] field so the UI can render a muted hint next to
//! the label.
//!
//! See `plan/3-block-detail.md` §12.4.

use crate::{
    application::ports::LabelPort,
    domain::{Address, Chain, DomainError, Label},
};

#[derive(Debug, Clone)]
pub struct CompositeLabels<P, S>
where
    P: LabelPort + Clone,
    S: LabelPort + Clone,
{
    primary: P,
    secondary: S,
}

impl<P, S> CompositeLabels<P, S>
where
    P: LabelPort + Clone,
    S: LabelPort + Clone,
{
    #[must_use]
    pub fn new(primary: P, secondary: S) -> Self {
        Self { primary, secondary }
    }
}

impl<P, S> LabelPort for CompositeLabels<P, S>
where
    P: LabelPort + Clone + Send + Sync,
    S: LabelPort + Clone + Send + Sync,
{
    async fn label_for(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<Option<Label>, DomainError> {
        if let Some(hit) = self.primary.label_for(address, chain).await? {
            return Ok(Some(hit));
        }
        self.secondary.label_for(address, chain).await
    }
}
