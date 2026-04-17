//! Use case: load the full Transaction entity shown on the Overview
//! tab of the TxDetail screen.
//!
//! See `plan/4-tx-detail.md` section 12.1.

use crate::{
    application::ports::TxReaderPort,
    domain::{Chain, DomainError, Transaction, TxHash},
};

pub async fn run<P: TxReaderPort>(
    reader: &P,
    hash: TxHash,
    chain: Chain,
) -> Result<Transaction, DomainError> {
    match reader.get(hash, chain).await? {
        Some(tx) => Ok(tx),
        None => Err(DomainError::NotFound),
    }
}
