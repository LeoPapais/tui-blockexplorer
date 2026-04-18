//! ENS resolver adapter.
//!
//! Resolves forward and reverse records via `eth_call` through the RPC
//! adapter (`crate::adapters::rpc::AlchemyEnsResolver`). This module
//! hosts wrappers that compose on top of any `EnsResolverPort`, such
//! as the TTL cache decorator mandated by
//! `.cursor/rules/external-apis.mdc`.

pub mod cached;

pub use cached::{CachedEnsResolver, DEFAULT_REVERSE_TTL};
