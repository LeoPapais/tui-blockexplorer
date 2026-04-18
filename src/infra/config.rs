//! Configuration loader for credentials and defaults.
//!
//! Reads the environment first, then a TOML file at the XDG config
//! directory, and merges the two with env winning. See
//! `plan/14-config-and-credentials.md` section 2.

use std::path::PathBuf;

use directories::ProjectDirs;
use serde::Deserialize;

use crate::domain::{Chain, DomainError};

/// Names of env vars consulted in order of precedence.
pub const ENV_ALCHEMY_KEY: &str = "ALCHEMY_API_KEY";
pub const ENV_ETHERSCAN_KEY: &str = "ETHERSCAN_API_KEY";
pub const ENV_CHAIN: &str = "BLOCKEXPLORER_TUI_CHAIN";

/// Function type used by [`ConfigLoader`] to read the environment.
/// Factored as a type alias so clippy does not complain about the
/// complex `dyn Fn` inside the struct.
type EnvReader = dyn Fn(&str) -> Option<String> + Send + Sync;

/// High-level config value consumed by the composition root.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub credentials: ApiCredentials,
    pub chain: Chain,
}

#[derive(Debug, Clone, Default)]
pub struct ApiCredentials {
    pub alchemy: Option<String>,
    pub etherscan: Option<String>,
}

impl AppConfig {
    /// Convenience constructor used by the binary. Relies on
    /// [`ConfigLoader::from_env`] plus the default config-file
    /// location.
    pub fn load() -> Result<Self, DomainError> {
        ConfigLoader::default().load()
    }

    #[must_use]
    pub fn has_alchemy_key(&self) -> bool {
        self.credentials.alchemy.is_some()
    }
}

/// Configurable loader. Exists mostly so tests can inject a synthetic
/// environment and file path without touching the real filesystem.
pub struct ConfigLoader {
    env: Box<EnvReader>,
    config_path: Option<PathBuf>,
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self {
            env: Box::new(|key| std::env::var(key).ok()),
            config_path: default_config_path(),
        }
    }
}

impl ConfigLoader {
    /// Build a loader that consults the given environment map. The
    /// config-file path defaults to the XDG location.
    pub fn with_env<F>(env: F) -> Self
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        Self {
            env: Box::new(env),
            config_path: None,
        }
    }

    /// Override the config-file path. Useful in tests.
    #[must_use]
    pub fn with_config_path(mut self, path: Option<PathBuf>) -> Self {
        self.config_path = path;
        self
    }

    /// Run the loader and produce an [`AppConfig`].
    pub fn load(&self) -> Result<AppConfig, DomainError> {
        let file = self.load_file()?;

        let env_alchemy = (self.env)(ENV_ALCHEMY_KEY);
        let env_etherscan = (self.env)(ENV_ETHERSCAN_KEY);
        let env_chain = (self.env)(ENV_CHAIN);

        let alchemy = env_alchemy
            .or_else(|| file.credentials.as_ref().and_then(|c| c.alchemy.clone()));
        let etherscan = env_etherscan
            .or_else(|| file.credentials.as_ref().and_then(|c| c.etherscan.clone()));

        let chain = match env_chain.or(file.defaults.and_then(|d| d.chain)) {
            Some(slug) => Chain::from_slug(&slug)?,
            None => Chain::Ethereum,
        };

        Ok(AppConfig {
            credentials: ApiCredentials { alchemy, etherscan },
            chain,
        })
    }

    fn load_file(&self) -> Result<ConfigFile, DomainError> {
        let Some(path) = self.config_path.as_ref() else {
            return Ok(ConfigFile::default());
        };
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str::<ConfigFile>(&text)
                .map_err(|e| DomainError::Config(format!("malformed config {}: {e}", path.display()))),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(ConfigFile::default()),
            Err(err) => Err(DomainError::Config(format!(
                "failed to read {}: {err}",
                path.display()
            ))),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    credentials: Option<CredentialsFile>,
    defaults: Option<DefaultsFile>,
}

#[derive(Debug, Default, Deserialize)]
struct CredentialsFile {
    alchemy: Option<String>,
    etherscan: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct DefaultsFile {
    chain: Option<String>,
}

fn default_config_path() -> Option<PathBuf> {
    let dirs = ProjectDirs::from("", "", "blockexplorer-tui")?;
    Some(dirs.config_dir().join("config.toml"))
}

/// Test helpers exposed publicly so integration tests under `tests/`
/// can drive [`ConfigLoader`] without touching the real environment or
/// filesystem.
pub mod test_helpers {
    use std::sync::Arc;

    /// Build a closure usable as the `env` argument of
    /// [`super::ConfigLoader::with_env`] from a list of `(key, value)`
    /// pairs. Returns `None` for keys not in the list.
    pub fn env_map(
        pairs: Vec<(&'static str, &'static str)>,
    ) -> impl Fn(&str) -> Option<String> + Send + Sync + 'static {
        let pairs = Arc::new(pairs);
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| k == &key)
                .map(|(_, v)| (*v).to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_helpers::env_map;

    #[test]
    fn env_wins_over_missing_file() {
        let loader = ConfigLoader::with_env(env_map(vec![(ENV_ALCHEMY_KEY, "key-from-env")]));
        let cfg = loader.load().expect("ok");

        assert_eq!(cfg.credentials.alchemy.as_deref(), Some("key-from-env"));
        assert_eq!(cfg.chain, Chain::Ethereum);
    }

    #[test]
    fn env_chain_is_parsed() {
        let loader = ConfigLoader::with_env(env_map(vec![
            (ENV_ALCHEMY_KEY, "k"),
            (ENV_CHAIN, "base"),
        ]));
        let cfg = loader.load().expect("ok");

        assert_eq!(cfg.chain, Chain::Base);
    }

    #[test]
    fn unknown_chain_slug_is_rejected() {
        let loader = ConfigLoader::with_env(env_map(vec![(ENV_CHAIN, "nope")]));
        let err = loader.load().expect_err("unknown chain must error");

        assert!(matches!(err, DomainError::InvalidInput(_)));
    }
}
