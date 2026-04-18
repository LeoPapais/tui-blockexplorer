//! Integration-level tests for `ConfigLoader`, covering file parsing
//! and the env-file precedence rules.
//!
//! Uses `tempfile` via an ad-hoc `PathBuf` in the target dir — the
//! project does not depend on `tempfile` and the generated files are
//! tiny, so we write them under `target/test-config/` and clean up
//! afterwards.
//!
//! See `plan/14-config-and-credentials.md` section 5.

use std::{fs, path::PathBuf};

use blockexplorer_tui::{
    domain::{Chain, DomainError},
    infra::{
        ConfigLoader,
        config::{ENV_ALCHEMY_KEY, ENV_CHAIN, test_helpers::env_map},
    },
};

fn scratch_path(label: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("blockexplorer-tui-config");
    fs::create_dir_all(&dir).unwrap();
    dir.join(format!("{label}.toml"))
}

#[test]
fn env_alone_produces_config() {
    let loader =
        ConfigLoader::with_env(env_map(vec![(ENV_ALCHEMY_KEY, "env-key")])).with_config_path(None);
    let cfg = loader.load().expect("ok");

    assert_eq!(cfg.credentials.alchemy.as_deref(), Some("env-key"));
    assert_eq!(cfg.chain, Chain::Ethereum);
}

#[test]
fn file_alone_produces_config() {
    let path = scratch_path("only_file");
    fs::write(
        &path,
        "[credentials]\nalchemy = \"file-key\"\n[defaults]\nchain = \"base\"\n",
    )
    .unwrap();

    let loader = ConfigLoader::with_env(env_map(vec![])).with_config_path(Some(path.clone()));
    let cfg = loader.load().expect("ok");

    assert_eq!(cfg.credentials.alchemy.as_deref(), Some("file-key"));
    assert_eq!(cfg.chain, Chain::Base);

    let _ = fs::remove_file(path);
}

#[test]
fn env_beats_file() {
    let path = scratch_path("env_beats_file");
    fs::write(
        &path,
        "[credentials]\nalchemy = \"file-key\"\n[defaults]\nchain = \"base\"\n",
    )
    .unwrap();

    let loader = ConfigLoader::with_env(env_map(vec![
        (ENV_ALCHEMY_KEY, "env-key"),
        (ENV_CHAIN, "polygon"),
    ]))
    .with_config_path(Some(path.clone()));
    let cfg = loader.load().expect("ok");

    assert_eq!(cfg.credentials.alchemy.as_deref(), Some("env-key"));
    assert_eq!(cfg.chain, Chain::Polygon);

    let _ = fs::remove_file(path);
}

#[test]
fn missing_file_is_not_an_error() {
    let loader = ConfigLoader::with_env(env_map(vec![]))
        .with_config_path(Some(PathBuf::from("/nonexistent/path/to.toml")));
    let cfg = loader.load().expect("ok");

    assert!(cfg.credentials.alchemy.is_none());
    assert_eq!(cfg.chain, Chain::Ethereum);
}

#[test]
fn malformed_file_is_rejected() {
    let path = scratch_path("malformed");
    fs::write(&path, "this is not TOML }}} [broken").unwrap();

    let loader = ConfigLoader::with_env(env_map(vec![])).with_config_path(Some(path.clone()));
    let err = loader.load().expect_err("malformed TOML must error");

    assert!(matches!(err, DomainError::Config(_)));

    let _ = fs::remove_file(path);
}
