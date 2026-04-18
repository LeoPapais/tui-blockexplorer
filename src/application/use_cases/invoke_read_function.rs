//! Use case: invoke a read-only ABI function.
//!
//! See `plan/7-contract-detail.md` section 12.4.2.

use crate::{
    application::ports::ContractReaderPort,
    domain::{AbiFunction, AbiValue, Address, Chain, DecodedValue, DomainError},
};

pub async fn run<R: ContractReaderPort>(
    reader: &R,
    address: Address,
    chain: Chain,
    function: &AbiFunction,
    args: Vec<AbiValue>,
) -> Result<Vec<DecodedValue>, DomainError> {
    if !function.is_read_only {
        return Err(DomainError::InvalidInput(format!(
            "{name} is not read-only",
            name = function.name
        )));
    }
    if !function.is_executable() {
        return Err(DomainError::InvalidInput(
            "function has unsupported ABI input types".into(),
        ));
    }
    if args.len() != function.inputs.len() {
        return Err(DomainError::InvalidInput(format!(
            "expected {} arguments, got {}",
            function.inputs.len(),
            args.len(),
        )));
    }
    reader.call(address, chain, function, args).await
}
