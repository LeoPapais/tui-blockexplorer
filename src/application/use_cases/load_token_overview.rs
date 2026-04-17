//! Use case: load the Token Detail Overview view-model.
//!
//! See `plan/8-token-detail.md` section 12.1.

use crate::{
    application::ports::TokenReaderPort,
    domain::{Address, Chain, DomainError, TokenOverview},
};

pub async fn run<P: TokenReaderPort>(
    reader: &P,
    address: Address,
    chain: Chain,
) -> Result<TokenOverview, DomainError> {
    match reader.get(address, chain).await? {
        Some(overview) => Ok(overview),
        None => Err(DomainError::NotFound),
    }
}
