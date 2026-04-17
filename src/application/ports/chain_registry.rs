//! Outbound port exposing which chains the user has enabled in config.
//!
//! Synchronous because the data is memory-resident (loaded once at startup
//! from the config file). See `plan/1-home.md` section 4.3.

use crate::domain::{Chain, DomainError};

pub trait ChainRegistryPort: Send + Sync {
    /// List every chain currently enabled. Order matches the chain picker.
    fn list_enabled(&self) -> Vec<Chain>;

    /// The chain the app boots into.
    fn default_chain(&self) -> Chain;

    /// Validate that the caller can switch to `chain`. Returns
    /// [`DomainError::FeatureUnavailable`] if the chain is disabled in the
    /// user's config.
    fn ensure_enabled(&self, chain: Chain) -> Result<(), DomainError>;
}
