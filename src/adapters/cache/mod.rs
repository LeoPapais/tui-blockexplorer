//! In-memory TTL cache adapters.
//!
//! The `memory::TtlCache<K, V>` type is a generic in-memory, TTL-scoped
//! store keyed by any hashable type. Used by the search feed (60 s per
//! resolved-entity list), the ENS reverse decorator (5 min) and the
//! Etherscan proxy-hint decorator (5 min).
//!
//! The TTL constants for each logical cache live in
//! [`registry::CacheRegistry`] so the composition root and the
//! decorators agree on the exact duration without having to copy a
//! `Duration::from_secs(…)` literal around. See
//! `plan/15-backlog.md` §8.16 "Cache registry".

pub mod memory;
pub mod registry;

pub use memory::TtlCache;
pub use registry::{ABI_TTL, CacheRegistry, ENS_REVERSE_TTL, HEALTH_TTL, SEARCH_TTL};
