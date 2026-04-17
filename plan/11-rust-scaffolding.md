# 11 — Rust Scaffolding

Not a screen. This is the mechanical work to stand up the Rust project so that
functional and e2e tests can start being written. Follows the rules in
[`.cursor/rules/architecture.mdc`](../.cursor/rules/architecture.mdc) and
[`.cursor/rules/testing.mdc`](../.cursor/rules/testing.mdc).

## 1. Toolchain

- `rust-toolchain.toml` pins channel `stable` with profile `default` and components
  `rustfmt`, `clippy`.
- Edition `2024` across the crate.
- No MSRV below the current stable at scaffold time.

## 2. Package layout

Single binary crate named `blockexplorer-tui`. No workspace yet: the whole app is
small enough that one crate plus integration tests is simpler. If and when
multiple binaries or separate library crates become necessary we switch to a
workspace in a separate plan.

```
Cargo.toml
rust-toolchain.toml
src/
  main.rs
  lib.rs                     # public root, re-exports domain/application
  domain/mod.rs              # empty placeholders
  application/
    mod.rs
    ports/mod.rs
    use_cases/mod.rs
  adapters/
    mod.rs
    rpc/mod.rs
    etherscan/mod.rs
    signatures/mod.rs
    ens/mod.rs
    cache/mod.rs
    ui/mod.rs
    config/mod.rs
  infra/
    mod.rs
tests/
  e2e.rs                     # cucumber test binary entry point
  e2e/
    features/                # .feature files
    steps/                   # step definitions modules
    world.rs                 # cucumber World (holds stub ports + screen stack)
  support/
    mod.rs
    fixture_loader.rs        # loads tests/fixtures/*.json
    stubs.rs                 # stub implementations of outbound ports
  fixtures/                  # {adapter}__{method}__{case}.json
```

`src/lib.rs` exists so integration tests under `tests/` can depend on the crate.

## 3. Dependencies

Added via `cargo add` so exact versions are resolved by cargo at the time of
scaffolding. Runtime dependencies:

- `tokio` (features: `rt-multi-thread`, `macros`, `sync`, `time`, `signal`,
  `io-std`, `io-util`).
- `ratatui`
- `crossterm`
- `reqwest` (features: `json`, `rustls-tls`, `stream`, `gzip`).
- `serde`, `serde_json`
- `thiserror`
- `anyhow` (only used at `main` / `infra` edges).
- `tracing`, `tracing-subscriber`
- `moka` (features: `future`) for the in-memory TTL cache adapter.
- `arboard` for clipboard.
- `directories` for config path resolution.
- `toml` for parsing the config file.
- `url` for URL building.
- `hex` and `bytes` for byte handling in domain types.

Dev dependencies:

- `cucumber` for BDD.
- `rstest` for parameterized functional tests.
- `pretty_assertions`
- `assert_matches`
- `tokio` with the `test-util` feature.
- `wiremock` for HTTP stubs in adapter-level tests (optional, not used in e2e).

## 4. Test harness

- `tests/e2e.rs` is a regular Rust integration test binary that runs all
  `*.feature` files under `tests/e2e/features/` using `cucumber::World`.
- The `World` struct lives in `tests/e2e/world.rs` and holds:
  - an instance of every stub port (see `tests/support/stubs.rs`),
  - an in-memory `ScreenStack` (driven at the application level, not through
    the real TUI backend),
  - the last rendered frame captured via `ratatui::backend::TestBackend` for
    visual assertions.
- Step definitions live in `tests/e2e/steps/{screen}.rs`. Each file exposes
  `#[given] / #[when] / #[then]` functions registered through
  `cucumber::given` / `cucumber::when` / `cucumber::then` attributes.
- Fixture loader at `tests/support/fixture_loader.rs` exposes
  `load_json<T: DeserializeOwned>(relative_path: &str) -> T`, reading files
  from `tests/fixtures/`. It panics on missing files because a missing fixture
  is always a test bug.

## 5. Red-first delivery

The first concrete deliverable after the scaffolding is the Home feature,
deliberately red:

- `tests/e2e/features/home.feature` contains the four scenarios from
  [`plan/1-home.md`](1-home.md) section 7.
- Step definitions call into the application layer. Since the application
  layer is still empty, every step that asserts behaviour panics with
  `todo!("see plan/1-home.md#...")`. The test binary must still compile; only
  the runtime assertions fail.
- No production code has been written at this point.

## 6. Acceptance

- `cargo check` passes.
- `cargo test --test e2e -- --help` prints cucumber usage without build
  errors.
- `cargo test --test e2e` runs and reports every Home scenario as failed.
- `cargo clippy --all-targets` produces no errors (warnings are allowed at
  this stage since modules are intentionally empty).

## 7. Out of scope for this plan

- Any production code.
- CI configuration.
- Pre-commit hooks.
- Fixture files beyond the minimum needed to make Home scenarios parseable
  (fixtures land together with functional tests, not with scaffolding).

## 8. Next steps (separate plans)

- Phase 3a: write the Home functional tests (red), then the minimal
  implementation to make them green.
- Phase 3b: repeat for Search, then for each MVP screen in the order of the
  plan index.
