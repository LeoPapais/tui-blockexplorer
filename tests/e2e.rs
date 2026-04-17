//! E2E (BDD) test binary.
//!
//! Runs every `*.feature` file under `tests/e2e/features/` through the
//! cucumber-rs harness. Step definitions live under `tests/e2e/steps/`.
//! The shared `AppWorld` lives in `tests/e2e/world.rs`.
//!
//! This binary is declared with `harness = false` in `Cargo.toml` so that
//! cucumber can install its own runner.

#[path = "support/mod.rs"]
mod support;

#[path = "e2e/world.rs"]
mod world;

#[path = "e2e/steps/mod.rs"]
mod steps;

use cucumber::World;

#[tokio::main]
async fn main() {
    world::AppWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit("tests/e2e/features")
        .await;
}
