//! Use case: paginated transfers for one ERC-20 contract.
//!
//! See `plan/8-token-detail.md` section 4.2.

use crate::{
    application::ports::TransfersPort,
    domain::{Address, Chain, DomainError, TransferCursor, TransferPage},
};

pub async fn run<T: TransfersPort>(
    transfers: &T,
    contract: Address,
    chain: Chain,
    cursor: Option<TransferCursor>,
) -> Result<TransferPage, DomainError> {
    transfers.get_for_contract(contract, chain, cursor).await
}
