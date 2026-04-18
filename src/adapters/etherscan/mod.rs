//! Etherscan V2 REST adapter.
//!
//! Implements `ContractSourcePort` today; the rest of the port roster
//! (labels, raw source files, multi-file verification payloads) will
//! land as the Source / Labels tabs are implemented across the
//! remaining plan files.

pub mod client;
pub mod contract_source;

pub use client::{EtherscanClient, EtherscanError};
pub use contract_source::EtherscanContractSource;
