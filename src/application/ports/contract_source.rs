//! Outbound port exposing Etherscan source / ABI data for a contract.
//!
//! See `plan/4-tx-detail.md` section 12.4.1.

use crate::domain::{Address, Chain, ContractAbi, DomainError};

pub trait ContractSourcePort: Send + Sync {
    /// Fetch the ABI metadata for `address` on `chain`. Returns
    /// `Ok(None)` when the contract is unverified or no key is
    /// configured. Provider failures surface as `Err(DomainError::*)`.
    fn get_abi(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<ContractAbi>, DomainError>> + Send;
}
