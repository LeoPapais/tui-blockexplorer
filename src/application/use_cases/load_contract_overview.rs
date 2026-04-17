//! Use case: load the Contract Detail Overview view-model.
//!
//! Combines `AddressReaderPort` (account basics) with
//! `ProxyDetectionPort` (optional EIP-1967 proxy metadata). See
//! `plan/7-contract-detail.md` section 12.1.

use crate::{
    application::ports::{AddressReaderPort, ProxyDetectionPort},
    domain::{Address, Chain, ContractOverview, DomainError},
};

pub async fn run<A, P>(
    address_reader: &A,
    proxy_detector: &P,
    address: Address,
    chain: Chain,
) -> Result<ContractOverview, DomainError>
where
    A: AddressReaderPort,
    P: ProxyDetectionPort,
{
    let (account_res, proxy_res) = tokio::join!(
        address_reader.get(address, chain),
        proxy_detector.detect(address, chain),
    );

    let account = match account_res? {
        Some(overview) => overview,
        None => return Err(DomainError::NotFound),
    };
    // Proxy-detection failures are non-fatal: the overview is still
    // useful without proxy information.
    let proxy = proxy_res.ok().flatten();

    Ok(ContractOverview { account, proxy })
}
