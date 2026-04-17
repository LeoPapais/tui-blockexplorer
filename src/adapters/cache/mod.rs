//! In-memory TTL cache adapter built on `moka`.
//!
//! Exposes a generic `CachePort` used by other adapters to memoize
//! responses with short TTLs. No persistence.
