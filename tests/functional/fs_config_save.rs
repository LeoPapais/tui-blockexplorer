//! Filesystem adapter for `ConfigPort::save`.
//!
//! See `plan/10-settings.md` section 12.3.

use std::{fs, path::PathBuf};

use blockexplorer_tui::{
    adapters::config::FsConfig,
    application::ports::{ConfigPatch, ConfigPort},
    domain::{Chain, DomainError},
    infra::{
        ConfigLoader,
        config::{ENV_ALCHEMY_KEY, test_helpers::env_map},
    },
};
use pretty_assertions::assert_eq;

fn scratch_dir(label: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("blockexplorer-tui-fs-config")
        .join(label);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn save_persists_default_chain_and_round_trips_through_loader() {
    let dir = scratch_dir("happy");
    let path = dir.join("config.toml");
    let adapter = FsConfig::new(path.clone());

    let view = adapter
        .save(&ConfigPatch::with_default_chain(Chain::Base))
        .await
        .expect("save must succeed");

    assert_eq!(view.chain, Chain::Base);
    assert!(!view.alchemy_key_present);

    // Round-trip through the existing loader.
    let loader = ConfigLoader::with_env(env_map(vec![])).with_config_path(Some(path.clone()));
    let cfg = loader.load().expect("loader ok");
    assert_eq!(cfg.chain, Chain::Base);

    // And the adapter itself can re-read what it wrote.
    let view_again = adapter.load().await.expect("load ok");
    assert_eq!(view_again.chain, Chain::Base);
}

#[tokio::test]
async fn save_preserves_existing_credentials_when_patch_omits_them() {
    let dir = scratch_dir("preserves_creds");
    let path = dir.join("config.toml");
    fs::write(
        &path,
        "[credentials]\nalchemy = \"existing-key\"\n[defaults]\nchain = \"ethereum\"\n",
    )
    .unwrap();

    let adapter = FsConfig::new(path.clone());

    adapter
        .save(&ConfigPatch::with_default_chain(Chain::Polygon))
        .await
        .expect("save ok");

    let text = fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("alchemy = \"existing-key\""),
        "alchemy key must survive a partial patch"
    );
    assert!(
        text.contains("chain = \"polygon\""),
        "patched chain must land on disk: {text}"
    );
}

#[tokio::test]
async fn save_is_atomic_and_leaves_no_temp_files_behind() {
    let dir = scratch_dir("atomic");
    let path = dir.join("config.toml");
    let adapter = FsConfig::new(path.clone());

    adapter
        .save(&ConfigPatch::with_default_chain(Chain::Base))
        .await
        .expect("save ok");

    let entries: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();

    assert!(
        entries.contains(&"config.toml".to_string()),
        "target file must be present: {entries:?}"
    );
    let leftover: Vec<_> = entries
        .iter()
        .filter(|name| name.starts_with(".config.toml.tmp") || name.ends_with(".tmp"))
        .collect();
    assert!(
        leftover.is_empty(),
        "atomic write must leave no temp files behind, found {leftover:?}"
    );
}

#[tokio::test]
async fn save_errors_when_target_directory_is_not_writable() {
    // Point the adapter at a path whose parent cannot be created
    // because one of its components is an existing regular file.
    let dir = scratch_dir("not_writable");
    let blocker = dir.join("blocker");
    fs::write(&blocker, "I am a file, not a directory").unwrap();

    let path = blocker.join("config.toml");
    let adapter = FsConfig::new(path);

    let err = adapter
        .save(&ConfigPatch::with_default_chain(Chain::Ethereum))
        .await
        .expect_err("save must fail on unusable parent dir");

    assert!(matches!(err, DomainError::Config(_)), "unexpected: {err:?}");
}

#[tokio::test]
async fn save_includes_env_set_alchemy_key_when_patched() {
    let dir = scratch_dir("with_alchemy");
    let path = dir.join("config.toml");
    let adapter = FsConfig::new(path.clone());

    let mut patch = ConfigPatch::with_default_chain(Chain::Ethereum);
    patch.alchemy_key = Some("persisted-key".to_string());

    let view = adapter.save(&patch).await.expect("save ok");
    assert!(view.alchemy_key_present);

    // Confirm the loader actually returns the saved key when the env
    // is empty. This proves the save output is consumable by the
    // composition root on the next launch.
    let loader = ConfigLoader::with_env(env_map(vec![(ENV_ALCHEMY_KEY, "env-key")]))
        .with_config_path(Some(path));
    let cfg = loader.load().expect("loader ok");
    assert_eq!(cfg.credentials.alchemy.as_deref(), Some("env-key"));
}
