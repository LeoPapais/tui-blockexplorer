//! Plaintext `SecretStorePort` adapter.
//!
//! Delegates to a [`ConfigPort`] so writes are persisted via the
//! atomic `save` path. Emits a single warning at construction so the
//! user is reminded the keys live unencrypted on disk. The warning
//! runs through the masking logger from
//! `plan/10-settings.md` §12.1 so the value itself is never echoed.
//!
//! OS-keychain integration is deliberately deferred; see §12.6.

use std::sync::Arc;

use crate::{
    application::ports::{
        AppConfigView, ConfigPatch, ConfigPort, SecretStorePort, secret_keys,
    },
    domain::DomainError,
};

/// Adapter backed by a [`ConfigPort`]. Every `set` call triggers a
/// `ConfigPort::save` with the matching field set, keeping the
/// `config.toml` file the single source of truth.
pub struct PlaintextSecretStore<C: ConfigPort> {
    config: Arc<C>,
}

impl<C: ConfigPort> PlaintextSecretStore<C> {
    /// Build the adapter around an existing config port. Logs a
    /// single warning so the user knows the backing store is
    /// plaintext. The warning is emitted via `tracing::warn!` so the
    /// masking logger from §12.1 scrubs any contextual field that
    /// happens to carry a key / token / secret.
    #[must_use]
    pub fn new(config: Arc<C>) -> Self {
        tracing::warn!(
            target = "blockexplorer_tui::secrets",
            "SecretStore: plaintext backend in use, keys are stored unencrypted in config.toml",
        );
        Self { config }
    }
}

impl<C: ConfigPort> SecretStorePort for PlaintextSecretStore<C> {
    async fn get(&self, key: &str) -> Result<Option<String>, DomainError> {
        // For the MVP the adapter only exposes presence through the
        // `AppConfigView`. The config file itself carries the value;
        // callers that need the plaintext still go through
        // `infra::config::ConfigLoader` (which the composition root
        // already uses). Presence is enough for the Settings screen,
        // which is the sole current consumer.
        let view = self.config.load().await?;
        let present = match key {
            k if k == secret_keys::ALCHEMY_API_KEY => view.alchemy_key_present,
            k if k == secret_keys::ETHERSCAN_API_KEY => view.etherscan_key_present,
            _ => false,
        };
        if present {
            // We deliberately do not return the real value here so
            // adapters that only need to know *whether* a key is set
            // (e.g. the Settings screen) are not tempted to log it.
            Ok(Some("<redacted>".to_string()))
        } else {
            Ok(None)
        }
    }

    async fn set(&self, key: &str, value: &str) -> Result<(), DomainError> {
        let mut patch = ConfigPatch::default();
        match key {
            k if k == secret_keys::ALCHEMY_API_KEY => patch.alchemy_key = Some(value.to_string()),
            k if k == secret_keys::ETHERSCAN_API_KEY => {
                patch.etherscan_key = Some(value.to_string())
            }
            other => {
                return Err(DomainError::InvalidInput(format!(
                    "unknown secret key: {other}"
                )));
            }
        }
        let _view: AppConfigView = self.config.save(&patch).await?;
        Ok(())
    }
}
