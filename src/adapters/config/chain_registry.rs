//! Minimal in-memory [`ChainRegistryPort`] implementation used by the
//! composition root until the Settings screen lands.
//!
//! See `plan/14-config-and-credentials.md` section 4.

use crate::{
    application::ports::ChainRegistryPort,
    domain::{Chain, DomainError},
};

/// Registry backed by a hardcoded list of chains and a default picked
/// from [`AppConfig`](crate::infra::AppConfig).
#[derive(Debug, Clone)]
pub struct InMemoryChainRegistry {
    enabled: Vec<Chain>,
    default_chain: Chain,
}

impl InMemoryChainRegistry {
    /// Build a registry with every known chain enabled and `default`
    /// as the boot chain.
    #[must_use]
    pub fn with_default(default: Chain) -> Self {
        Self {
            enabled: Chain::all().to_vec(),
            default_chain: default,
        }
    }
}

impl ChainRegistryPort for InMemoryChainRegistry {
    fn list_enabled(&self) -> Vec<Chain> {
        self.enabled.clone()
    }

    fn default_chain(&self) -> Chain {
        self.default_chain
    }

    fn ensure_enabled(&self, chain: Chain) -> Result<(), DomainError> {
        if self.enabled.contains(&chain) {
            Ok(())
        } else {
            Err(DomainError::FeatureUnavailable)
        }
    }
}
