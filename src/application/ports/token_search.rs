//! Outbound port for token search by symbol or free text.
//!
//! Implementations live in the Etherscan adapter (future) or in a
//! hardcoded local list of popular tokens. See `plan/2-search.md`
//! section 4.

use crate::domain::{Chain, DomainError, TokenMetadata};

pub trait TokenSearchPort: Send + Sync {
    fn by_symbol(
        &self,
        symbol: &str,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Vec<TokenMetadata>, DomainError>> + Send;

    fn by_name(
        &self,
        text: &str,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Vec<TokenMetadata>, DomainError>> + Send;
}
