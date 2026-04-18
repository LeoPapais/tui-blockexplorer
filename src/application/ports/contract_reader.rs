//! Outbound port issuing read-only ABI calls (`eth_call`) against a
//! contract.
//!
//! See `plan/7-contract-detail.md` section 12.4.2.

use crate::domain::{AbiFunction, AbiValue, Address, Chain, DecodedValue, DomainError};

pub trait ContractReaderPort: Send + Sync {
    /// Encode `args` against `function`, issue `eth_call` at tag
    /// `latest` and return the decoded outputs. Reverts surface as
    /// `DomainError::ExecutionReverted { reason }`.
    fn call(
        &self,
        address: Address,
        chain: Chain,
        function: &AbiFunction,
        args: Vec<AbiValue>,
    ) -> impl std::future::Future<Output = Result<Vec<DecodedValue>, DomainError>> + Send;
}
