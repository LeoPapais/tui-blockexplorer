//! Outbound port exposing ENS forward and reverse resolution.
//!
//! See `plan/2-search.md` section 4 and the ENS rule in
//! `.cursor/rules/external-apis.mdc`.

use crate::domain::{Address, Chain, DomainError};

pub trait EnsResolverPort: Send + Sync {
    /// Forward resolution: from name to address.
    fn forward(
        &self,
        name: &str,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<Address>, DomainError>> + Send;

    /// Reverse resolution: from address to name.
    fn reverse(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<String>, DomainError>> + Send;
}
