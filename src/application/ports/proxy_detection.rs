//! Outbound port for EIP-1967 (and future) proxy detection.
//!
//! See `plan/7-contract-detail.md` section 12.1.

use crate::domain::{Address, Chain, DomainError, ProxyInfo};

pub trait ProxyDetectionPort: Send + Sync {
    /// Detect the proxy pattern at `address` on `chain`. Returns
    /// `Ok(None)` when no recognised proxy slot is populated.
    fn detect(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<ProxyInfo>, DomainError>> + Send;
}
