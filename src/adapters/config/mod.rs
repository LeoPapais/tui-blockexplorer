//! Config adapter.
//!
//! Hosts ChainRegistry implementations and (in a later plan) the
//! real TOML config adapter that implements `ConfigPort`.

pub mod chain_registry;

pub use chain_registry::InMemoryChainRegistry;
