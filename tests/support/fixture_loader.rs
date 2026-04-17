//! Load canned fixture files from `tests/fixtures/`.
//!
//! Fixtures are named using the convention `{adapter}__{method}__{case}.json`
//! as mandated by `.cursor/rules/testing.mdc`.

use std::{fs, path::PathBuf};

use serde::de::DeserializeOwned;

/// Root of the fixture directory, resolved at compile time from the
/// `CARGO_MANIFEST_DIR` environment variable.
fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Load a fixture file and deserialize it into `T`.
///
/// Panics if the file is missing or malformed: a missing fixture is always
/// a test bug, never a runtime condition we want to recover from.
#[allow(dead_code)]
pub fn load_json<T: DeserializeOwned>(relative_path: &str) -> T {
    let path = fixtures_root().join(relative_path);
    let bytes = fs::read(&path)
        .unwrap_or_else(|e| panic!("fixture not found at {}: {e}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("fixture {} is not valid JSON: {e}", path.display()))
}

/// Load a fixture file as a raw UTF-8 string. Useful for Gherkin tables
/// where the value is compared verbatim.
#[allow(dead_code)]
pub fn load_text(relative_path: &str) -> String {
    let path = fixtures_root().join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture not found at {}: {e}", path.display()))
}
