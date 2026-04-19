//! Config adapter.
//!
//! Hosts ChainRegistry implementations and (in a later plan) the
//! real TOML config adapter that implements `ConfigPort`.

pub mod chain_registry;
pub mod fs;

pub use chain_registry::InMemoryChainRegistry;
pub use fs::FsConfig;
