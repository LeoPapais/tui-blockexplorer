//! Outbound port traits.
//!
//! One file per capability. Each trait is documented with a link to the plan
//! section that introduced it.

pub mod chain_registry;
pub mod gas_oracle;
pub mod network_status;

pub use chain_registry::ChainRegistryPort;
pub use gas_oracle::GasOraclePort;
pub use network_status::NetworkStatusPort;
