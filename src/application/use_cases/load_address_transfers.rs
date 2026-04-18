//! Use case: load one page of transfers for an address.
//!
//! See `plan/6-address-detail.md` section 12.4.1.

use crate::{
    application::ports::TransfersPort,
    domain::{Address, Chain, DomainError, TransferCursor, TransferPage},
};

pub async fn run<T: TransfersPort>(
    transfers: &T,
    address: Address,
    chain: Chain,
    cursor: Option<TransferCursor>,
) -> Result<TransferPage, DomainError> {
    transfers.get_for_address(address, chain, cursor).await
}
