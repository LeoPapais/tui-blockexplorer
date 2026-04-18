//! Outbound port exposing a *hint* of the implementation behind a
//! proxy, as reported by a verified-sources provider (Etherscan
//! today). Used as a fallback by the composite proxy detector when
//! the slot-based [`super::ProxyDetectionPort`] returns `None`.
//!
//! See `plan/7-contract-detail.md` section 12.5.1.

use crate::domain::{Address, Chain, DomainError};

/// Returns the proxy implementation address that the upstream
/// source provider associates with `address`, or `None` when the
/// provider does not flag the contract as a proxy (or no key is
/// configured).
pub trait EtherscanProxyHintPort: Send + Sync {
    fn implementation_hint(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<Address>, DomainError>> + Send;
}
