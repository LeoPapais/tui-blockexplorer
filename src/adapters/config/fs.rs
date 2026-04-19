//! Filesystem-backed `ConfigPort` adapter.
//!
//! Writes go through a temp-file-and-rename dance so readers never
//! observe a partial write. Reads delegate to the existing
//! [`ConfigLoader`] so the load path stays in one place.
//!
//! See `plan/10-settings.md` section 12.3.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    application::ports::{AppConfigView, ConfigPatch, ConfigPort},
    domain::{Chain, DomainError},
    infra::config::{ENV_ALCHEMY_KEY, ENV_CHAIN, ENV_ETHERSCAN_KEY},
};

/// `ConfigPort` adapter that persists to a TOML file on disk.
pub struct FsConfig {
    path: PathBuf,
    /// Serialises concurrent writes so the temp-file-and-rename dance
    /// stays correct when multiple tasks save at once.
    write_lock: Mutex<()>,
}

impl FsConfig {
    /// Build an adapter that reads and writes to `path`. The parent
    /// directory is created lazily on the first `save`.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            write_lock: Mutex::new(()),
        }
    }

    /// Path this adapter reads from / writes to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn load_file(&self) -> Result<ConfigDocument, DomainError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => toml::from_str::<ConfigDocument>(&text).map_err(|err| {
                DomainError::Config(format!("malformed config {}: {err}", self.path.display()))
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(ConfigDocument::default()),
            Err(err) => Err(DomainError::Config(format!(
                "failed to read {}: {err}",
                self.path.display()
            ))),
        }
    }

    fn merge(&self, mut doc: ConfigDocument, patch: &ConfigPatch) -> ConfigDocument {
        if let Some(chain) = patch.default_chain {
            let defaults = doc.defaults.get_or_insert_with(Default::default);
            defaults.chain = Some(chain.slug().to_string());
        }
        if let Some(key) = patch.alchemy_key.as_ref() {
            let creds = doc.credentials.get_or_insert_with(Default::default);
            creds.alchemy = Some(key.clone());
        }
        if let Some(key) = patch.etherscan_key.as_ref() {
            let creds = doc.credentials.get_or_insert_with(Default::default);
            creds.etherscan = Some(key.clone());
        }
        doc
    }

    fn atomic_write(&self, doc: &ConfigDocument) -> Result<(), DomainError> {
        let rendered = toml::to_string_pretty(doc)
            .map_err(|err| DomainError::Config(format!("failed to render config TOML: {err}")))?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                DomainError::Config(format!(
                    "failed to create parent dir {}: {err}",
                    parent.display()
                ))
            })?;
        }

        let temp_path = temp_sibling(&self.path);

        // Guard against concurrent writes to the same target.
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| DomainError::Config("config write lock poisoned".to_string()))?;

        {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&temp_path)
                .map_err(|err| {
                    DomainError::Config(format!(
                        "failed to open temp file {}: {err}",
                        temp_path.display()
                    ))
                })?;
            file.write_all(rendered.as_bytes()).map_err(|err| {
                DomainError::Config(format!(
                    "failed to write temp file {}: {err}",
                    temp_path.display()
                ))
            })?;
            file.sync_all().map_err(|err| {
                DomainError::Config(format!(
                    "failed to fsync temp file {}: {err}",
                    temp_path.display()
                ))
            })?;
        }

        fs::rename(&temp_path, &self.path).map_err(|err| {
            // Best-effort cleanup.
            let _ = fs::remove_file(&temp_path);
            DomainError::Config(format!(
                "failed to rename {} -> {}: {err}",
                temp_path.display(),
                self.path.display()
            ))
        })?;

        Ok(())
    }
}

impl ConfigPort for FsConfig {
    async fn load(&self) -> Result<AppConfigView, DomainError> {
        let doc = self.load_file()?;
        into_view(&doc)
    }

    async fn save(&self, patch: &ConfigPatch) -> Result<AppConfigView, DomainError> {
        let current = self.load_file()?;
        let merged = self.merge(current, patch);
        self.atomic_write(&merged)?;
        into_view(&merged)
    }
}

fn temp_sibling(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|os| os.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config.toml".to_string());
    let pid = std::process::id();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let temp_name = format!(".{file_name}.tmp.{pid}.{ts}");
    match path.parent() {
        Some(parent) => parent.join(temp_name),
        None => PathBuf::from(temp_name),
    }
}

fn into_view(doc: &ConfigDocument) -> Result<AppConfigView, DomainError> {
    let chain_slug = doc
        .defaults
        .as_ref()
        .and_then(|d| d.chain.clone())
        .unwrap_or_else(|| Chain::Ethereum.slug().to_string());
    let chain = Chain::from_slug(&chain_slug)?;

    let alchemy_key_present = doc
        .credentials
        .as_ref()
        .and_then(|c| c.alchemy.as_ref())
        .is_some();
    let etherscan_key_present = doc
        .credentials
        .as_ref()
        .and_then(|c| c.etherscan.as_ref())
        .is_some();

    Ok(AppConfigView {
        chain,
        alchemy_key_present,
        etherscan_key_present,
    })
}

// ---------------------------------------------------------------------------
// Wire format
// ---------------------------------------------------------------------------

/// Mirror of `src/infra/config.rs::ConfigFile` — kept local to the
/// adapter so the write path owns serde derives.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct ConfigDocument {
    #[serde(skip_serializing_if = "Option::is_none")]
    credentials: Option<CredentialsDoc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    defaults: Option<DefaultsDoc>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct CredentialsDoc {
    #[serde(skip_serializing_if = "Option::is_none")]
    alchemy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    etherscan: Option<String>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct DefaultsDoc {
    #[serde(skip_serializing_if = "Option::is_none")]
    chain: Option<String>,
}

/// Env-variable names consumed by the loader side of the adapter.
/// Re-exported to keep call sites that bundle save + env loads in one
/// module (e.g. the runtime wiring) free of an extra import from
/// `infra`.
pub const ENV_NAMES: [&str; 3] = [ENV_ALCHEMY_KEY, ENV_ETHERSCAN_KEY, ENV_CHAIN];
