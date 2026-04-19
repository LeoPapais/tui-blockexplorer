//! Named TTL registry for the in-memory cache namespaces.
//!
//! `.cursor/rules/external-apis.mdc` and `plan/15-backlog.md` §8.16
//! call out a specific TTL per logical cache:
//!
//! | Namespace     | TTL      | Consumer                                           |
//! |---------------|----------|----------------------------------------------------|
//! | `ens_reverse` | 5 min    | `adapters::ens::CachedEnsResolver` (ENS reverse)   |
//! | `abi`         | 5 min    | Etherscan proxy hint / future ABI cache            |
//! | `search`      | 60 s     | `infra::search_feed::SearchCache`                  |
//! | `health`      | 30 s     | Health-port snapshots (future slice)               |
//!
//! Every consumer used to hard-code its own `Duration::from_secs(…)`
//! literal. That worked until we needed to touch more than one TTL in
//! a single review, at which point contributors had to grep for the
//! exact number and hope they caught every call site. The
//! [`CacheRegistry`] is the single source of truth: a plain struct of
//! named [`Duration`]s, cheap to clone, with `const` builders for
//! the defaults so the composition root can keep producing caches
//! without extra allocation.
//!
//! The registry deliberately does **not** own the cache instances —
//! consumers still build their own `TtlCache` around their key/value
//! types. It only provides the TTL values so the moment someone
//! changes "search TTL = 60 s" to "search TTL = 90 s", every consumer
//! picks it up automatically.

use std::time::Duration;

/// Default TTL for the ENS reverse cache (5 minutes).
pub const ENS_REVERSE_TTL: Duration = Duration::from_secs(300);

/// Default TTL for ABI / proxy-hint caches (5 minutes).
pub const ABI_TTL: Duration = Duration::from_secs(300);

/// Default TTL for the universal-search `ResolvedEntity` cache (60 s).
pub const SEARCH_TTL: Duration = Duration::from_secs(60);

/// Default TTL for health snapshots (30 s). Reserved for the
/// `HealthPort` snapshot cache once §8.11 promotes the health
/// routing.
pub const HEALTH_TTL: Duration = Duration::from_secs(30);

/// Named TTL registry passed around the composition root.
///
/// Holds one [`Duration`] per logical namespace. Cheap to clone and
/// safe to share across tasks — the struct is `Copy`.
///
/// ```ignore
/// use blockexplorer_tui::adapters::cache::CacheRegistry;
///
/// let registry = CacheRegistry::default();
/// assert_eq!(registry.search, std::time::Duration::from_secs(60));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct CacheRegistry {
    /// TTL for the ENS reverse cache.
    pub ens_reverse: Duration,
    /// TTL for the ABI / Etherscan proxy-hint cache.
    pub abi: Duration,
    /// TTL for the universal-search result cache.
    pub search: Duration,
    /// TTL for health-port snapshots.
    pub health: Duration,
}

impl CacheRegistry {
    /// The default registry used by the production composition root.
    ///
    /// Keeping this as an inherent `const` helper rather than a
    /// `Default` impl means we can use it in `const` contexts (for
    /// example when wiring static test registries).
    #[must_use]
    pub const fn production() -> Self {
        Self {
            ens_reverse: ENS_REVERSE_TTL,
            abi: ABI_TTL,
            search: SEARCH_TTL,
            health: HEALTH_TTL,
        }
    }
}

impl Default for CacheRegistry {
    fn default() -> Self {
        Self::production()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_registry_matches_public_ttl_constants() {
        let registry = CacheRegistry::production();
        assert_eq!(registry.ens_reverse, ENS_REVERSE_TTL);
        assert_eq!(registry.abi, ABI_TTL);
        assert_eq!(registry.search, SEARCH_TTL);
        assert_eq!(registry.health, HEALTH_TTL);
    }

    #[test]
    fn ttl_constants_match_external_api_contract() {
        // Locked-in values from `.cursor/rules/external-apis.mdc` and
        // `plan/15-backlog.md` §8.16. Touching one of these requires
        // updating the contract table in the module docstring and the
        // related plan sections.
        assert_eq!(ENS_REVERSE_TTL, Duration::from_secs(5 * 60));
        assert_eq!(ABI_TTL, Duration::from_secs(5 * 60));
        assert_eq!(SEARCH_TTL, Duration::from_secs(60));
        assert_eq!(HEALTH_TTL, Duration::from_secs(30));
    }

    #[test]
    fn default_matches_production() {
        let default = CacheRegistry::default();
        let production = CacheRegistry::production();
        assert_eq!(default.ens_reverse, production.ens_reverse);
        assert_eq!(default.abi, production.abi);
        assert_eq!(default.search, production.search);
        assert_eq!(default.health, production.health);
    }
}
