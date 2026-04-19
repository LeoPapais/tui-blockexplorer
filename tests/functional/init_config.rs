//! `blockexplorer_tui::infra::init_config_at` composition helper.
//!
//! Mirrors the behaviour of `cargo run -- --init-config`: seed a
//! `config.toml` at the given path via `FsConfig::save` so the next
//! invocation finds a valid file even before the user edits it.
//!
//! See `plan/14-config-and-credentials.md` §8.3.

use std::{fs, path::PathBuf};

use blockexplorer_tui::{
    domain::{Chain, DomainError},
    infra::{ConfigLoader, config::test_helpers::env_map, init_config_at},
};
use pretty_assertions::assert_eq;

fn scratch_dir(label: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("blockexplorer-tui-init-config")
        .join(label);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn init_config_at_creates_seed_file_with_default_chain() {
    let dir = scratch_dir("seed");
    let path = dir.join("config.toml");

    let view = init_config_at(&path).expect("init_config must succeed");

    assert!(path.exists(), "seed file must exist at {}", path.display());
    assert_eq!(view.chain, Chain::Ethereum);
    assert!(!view.alchemy_key_present);

    // Round-trip through the reader so we know the seed is usable by
    // the composition root on the next launch.
    let loader = ConfigLoader::with_env(env_map(vec![])).with_config_path(Some(path.clone()));
    let cfg = loader.load().expect("loader reads the seed");
    assert_eq!(cfg.chain, Chain::Ethereum);
    assert!(cfg.credentials.alchemy.is_none());
}

#[test]
fn init_config_at_is_idempotent() {
    // Running the flag twice must not fail and must not overwrite
    // anything the user already added between runs.
    let dir = scratch_dir("idempotent");
    let path = dir.join("config.toml");

    init_config_at(&path).expect("first init ok");

    fs::write(
        &path,
        "[credentials]\nalchemy = \"user-edited-key\"\n[defaults]\nchain = \"base\"\n",
    )
    .expect("user edits file between runs");

    let view = init_config_at(&path).expect("second init must succeed");

    assert_eq!(view.chain, Chain::Base);
    assert!(view.alchemy_key_present);

    let text = fs::read_to_string(&path).expect("read config");
    assert!(
        text.contains("alchemy = \"user-edited-key\""),
        "user edits must survive: {text}"
    );
}

#[test]
fn init_config_at_rejects_unusable_parent() {
    // Same shape as fs_config_save::save_errors_when_target_directory_is_not_writable:
    // pass a parent that is a regular file.
    let dir = scratch_dir("not_writable");
    let blocker = dir.join("blocker");
    fs::write(&blocker, "not a directory").unwrap();

    let path = blocker.join("config.toml");
    let err = init_config_at(&path).expect_err("bad parent must error");

    assert!(matches!(err, DomainError::Config(_)), "unexpected: {err:?}");
}
