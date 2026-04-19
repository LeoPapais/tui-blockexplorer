//! Secret store port.
//!
//! Abstracts where secrets (today just the Alchemy key; later also
//! Etherscan and Openchain) live. MVP ships a plaintext adapter that
//! delegates to [`ConfigPort`](super::ConfigPort); OS-keychain
//! integration is an explicit follow-up.
//!
//! See `plan/10-settings.md` section 12.6.

use std::future::Future;

use crate::domain::DomainError;

/// Outbound port for reading / writing secrets.
pub trait SecretStorePort: Send + Sync {
    /// Retrieve the secret stored under `key`, if any.
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<String>, DomainError>> + Send;

    /// Write `value` under `key`. Adapters are responsible for
    /// persistence (or for refusing the write when the backing store
    /// is read-only).
    fn set(&self, key: &str, value: &str) -> impl Future<Output = Result<(), DomainError>> + Send;
}

/// Canonical key names used across the application. Adapters are
/// not required to interpret them; they are just strings.
pub mod keys {
    pub const ALCHEMY_API_KEY: &str = "alchemy_api_key";
    pub const ETHERSCAN_API_KEY: &str = "etherscan_api_key";
    pub const OPENCHAIN_API_KEY: &str = "openchain_api_key";
}
