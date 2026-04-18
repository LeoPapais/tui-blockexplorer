//! Composite [`ProxyDetectionPort`] that layers a slot-based primary
//! (the Alchemy detector) with an optional [`EtherscanProxyHintPort`]
//! fallback. See `plan/7-contract-detail.md` section 12.5.1.
//!
//! The primary is consulted first. When it returns `Ok(None)` and an
//! Etherscan hint is available, the composite issues a hint lookup
//! and surfaces the address under [`ProxyInfo::etherscan_hint`].
//! Errors from the primary short-circuit the lookup — the hint is a
//! best-effort enrichment, not a masking layer for infrastructure
//! problems. Errors from the hint port are swallowed (the composite
//! degrades to "no proxy detected") because an Etherscan outage
//! must never break contract detail.

use crate::{
    application::ports::{EtherscanProxyHintPort, ProxyDetectionPort},
    domain::{Address, Chain, DomainError, ProxyInfo},
};

/// Primary + optional fallback. Keeping the fallback `Option`
/// removes the need for a Noop hint type in composition.
#[derive(Debug, Clone)]
pub struct CompositeProxyDetector<P, H> {
    primary: P,
    hint: Option<H>,
}

impl<P, H> CompositeProxyDetector<P, H> {
    #[must_use]
    pub fn new(primary: P, hint: Option<H>) -> Self {
        Self { primary, hint }
    }

    /// Convenience constructor used by tests that never wire the
    /// hint fallback.
    #[must_use]
    pub fn primary_only(primary: P) -> Self {
        Self {
            primary,
            hint: None,
        }
    }
}

impl<P, H> ProxyDetectionPort for CompositeProxyDetector<P, H>
where
    P: ProxyDetectionPort,
    H: EtherscanProxyHintPort,
{
    async fn detect(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<Option<ProxyInfo>, DomainError> {
        if let Some(info) = self.primary.detect(address, chain).await? {
            return Ok(Some(info));
        }
        let Some(hint) = self.hint.as_ref() else {
            return Ok(None);
        };
        match hint.implementation_hint(address, chain).await {
            Ok(Some(addr)) => Ok(Some(ProxyInfo::etherscan_hint(addr))),
            Ok(None) => Ok(None),
            Err(_) => Ok(None),
        }
    }
}
