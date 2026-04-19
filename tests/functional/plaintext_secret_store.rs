//! Functional tests for `PlaintextSecretStore`.
//!
//! See `plan/10-settings.md` section 12.6.

use std::sync::{Arc, Mutex};

use blockexplorer_tui::{
    adapters::secrets::PlaintextSecretStore,
    application::ports::{AppConfigView, ConfigPatch, ConfigPort, SecretStorePort, secret_keys},
    domain::{Chain, DomainError},
};
use pretty_assertions::assert_eq;

/// In-memory stub `ConfigPort` so the secret-store tests stay
/// filesystem-free.
#[derive(Default)]
struct StubConfig {
    state: Mutex<StubState>,
}

#[derive(Default, Clone)]
struct StubState {
    view: Option<AppConfigView>,
    saves: Vec<ConfigPatch>,
}

impl StubConfig {
    fn view_or_default(&self) -> AppConfigView {
        self.state
            .lock()
            .unwrap()
            .view
            .clone()
            .unwrap_or(AppConfigView {
                chain: Chain::Ethereum,
                alchemy_key_present: false,
                etherscan_key_present: false,
            })
    }

    fn saves(&self) -> Vec<ConfigPatch> {
        self.state.lock().unwrap().saves.clone()
    }
}

impl ConfigPort for StubConfig {
    async fn load(&self) -> Result<AppConfigView, DomainError> {
        Ok(self.view_or_default())
    }

    async fn save(&self, patch: &ConfigPatch) -> Result<AppConfigView, DomainError> {
        let mut guard = self.state.lock().unwrap();
        guard.saves.push(patch.clone());
        let view = guard.view.get_or_insert(AppConfigView {
            chain: Chain::Ethereum,
            alchemy_key_present: false,
            etherscan_key_present: false,
        });
        if patch.alchemy_key.is_some() {
            view.alchemy_key_present = true;
        }
        if patch.etherscan_key.is_some() {
            view.etherscan_key_present = true;
        }
        if let Some(chain) = patch.default_chain {
            view.chain = chain;
        }
        Ok(view.clone())
    }
}

#[tokio::test]
async fn set_persists_alchemy_key_through_config_port() {
    let config = Arc::new(StubConfig::default());
    let store = PlaintextSecretStore::new(config.clone());

    store
        .set(secret_keys::ALCHEMY_API_KEY, "live-key")
        .await
        .expect("set ok");

    let saves = config.saves();
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].alchemy_key.as_deref(), Some("live-key"));
    assert!(saves[0].etherscan_key.is_none());
}

#[tokio::test]
async fn get_reports_presence_but_never_the_plaintext_value() {
    let config = Arc::new(StubConfig::default());
    let store = PlaintextSecretStore::new(config.clone());

    assert_eq!(store.get(secret_keys::ALCHEMY_API_KEY).await.unwrap(), None);

    store
        .set(secret_keys::ALCHEMY_API_KEY, "live-key")
        .await
        .expect("set");

    let got = store.get(secret_keys::ALCHEMY_API_KEY).await.unwrap();
    assert_eq!(got.as_deref(), Some("<redacted>"));
}

#[tokio::test]
async fn set_rejects_unknown_keys() {
    let config = Arc::new(StubConfig::default());
    let store = PlaintextSecretStore::new(config);

    let err = store
        .set("unknown_key", "value")
        .await
        .expect_err("unknown keys must fail");

    assert!(matches!(err, DomainError::InvalidInput(_)));
}
