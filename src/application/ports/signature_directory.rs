//! Outbound port for function / event signature lookups.
//!
//! Originally scoped for openchain.xyz; we settled on Sourcify's
//! 4byte mirror at `https://api.4byte.sourcify.dev` because that is
//! where the user pointed the plan. The API shape matches the
//! 4byte.directory standard.
//!
//! See `plan/4-tx-detail.md` section 12.4.1.

use crate::domain::DomainError;

pub trait SignatureDirectoryPort: Send + Sync {
    /// Resolve a 4-byte function selector (lowercased `0x` hex) into
    /// a canonical signature string such as
    /// `transfer(address,uint256)`. Returns `Ok(None)` when nothing
    /// matches.
    fn lookup_selector(
        &self,
        selector: [u8; 4],
    ) -> impl std::future::Future<Output = Result<Option<String>, DomainError>> + Send;

    /// Resolve a 32-byte event topic0 into the canonical event
    /// signature text, e.g. `Transfer(address,address,uint256)`.
    fn lookup_event_topic(
        &self,
        topic: [u8; 32],
    ) -> impl std::future::Future<Output = Result<Option<String>, DomainError>> + Send;
}
