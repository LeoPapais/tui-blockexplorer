//! Outbound port for the Token Detail Overview tab.
//!
//! See `plan/8-token-detail.md` section 12.1.

use crate::domain::{Address, Chain, DomainError, TokenOverview};

pub trait TokenReaderPort: Send + Sync {
    fn get(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<TokenOverview>, DomainError>> + Send;
}
