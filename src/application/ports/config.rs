//! Config port.
//!
//! Abstracts the read + write path of the application's TOML
//! configuration so use cases and UI flows can persist user choices
//! without importing the concrete `FsConfig` adapter.
//!
//! See `plan/10-settings.md` section 12.3.

use std::future::Future;

use crate::domain::{Chain, DomainError};

/// Read-only snapshot of the user configuration exposed to the
/// application layer. Intentionally minimal — additional fields are
/// added as Settings gains editable surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfigView {
    pub chain: Chain,
    pub alchemy_key_present: bool,
    pub etherscan_key_present: bool,
}

/// Partial patch accepted by [`ConfigPort::save`]. Every field is
/// optional and only set values are written; everything else stays
/// as-is in the TOML file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigPatch {
    pub default_chain: Option<Chain>,
    pub alchemy_key: Option<String>,
    pub etherscan_key: Option<String>,
}

impl ConfigPatch {
    /// Convenience builder when the caller only wants to change the
    /// default chain.
    #[must_use]
    pub fn with_default_chain(chain: Chain) -> Self {
        Self {
            default_chain: Some(chain),
            ..Self::default()
        }
    }
}

/// Outbound port covering both reads and writes of the persistent
/// configuration. Adapters implement the atomic temp-file-and-rename
/// contract documented in the plan.
pub trait ConfigPort: Send + Sync {
    /// Load the current configuration. Missing file returns a sensible
    /// default; malformed file returns `DomainError::Config`.
    fn load(&self) -> impl Future<Output = Result<AppConfigView, DomainError>> + Send;

    /// Merge `patch` into the current configuration and persist the
    /// result. Adapters MUST perform the write atomically (temp file
    /// + rename) so readers never observe a half-written file.
    fn save(
        &self,
        patch: &ConfigPatch,
    ) -> impl Future<Output = Result<AppConfigView, DomainError>> + Send;
}
