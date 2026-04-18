//! In-memory TTL cache adapters.
//!
//! The `memory::TtlCache<K, V>` type is a generic in-memory, TTL-scoped
//! store keyed by any hashable type. Used by the search feed (60 s per
//! resolved-entity list) and, in later slices, by ENS (5 min) and
//! Etherscan ABI (5 min) consumers. See `plan/2-search.md` section 12.3.

pub mod memory;

pub use memory::TtlCache;
