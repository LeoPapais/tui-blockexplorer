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
- Fixture files beyond the minimum needed to make Home scenarios parseable
  (fixtures land together with functional tests, not with scaffolding).

## 8. Next steps (separate plans)

- Phase 3a: write the Home functional tests (red), then the minimal
  implementation to make them green.
- Phase 3b: repeat for Search, then for each MVP screen in the order of the
  plan index.

## 9. Tooling and enforcement (§8.12 in `plan/15-backlog.md`)

The scaffolding is not "done" until the repo can mechanically enforce the
rules in `.cursor/rules/rust-style.mdc`, `.cursor/rules/testing.mdc` and
`.cursor/rules/architecture.mdc`. The artefacts below land under this plan.

### 9.1 Crate-level lints

- `src/lib.rs` and `src/main.rs` carry the crate-wide floor:

  ```rust
  #![deny(clippy::dbg_macro)]
  #![warn(clippy::todo)]
  ```

  `dbg!` never ships; `todo!()` is flagged so reviewers ask for a plan
  reference on every occurrence.
- Two module roots opt into `clippy::pedantic` (item 7 in §8.12):
  `src/domain/mod.rs` and `src/application/use_cases/mod.rs`. Those layers
  are the highest-leverage targets for API cleanliness and do not produce
  pedantic noise the way adapters do (adapters pull in serde / reqwest /
  ratatui idioms that clash with `must_use_candidate`, `missing_errors_doc`
  and friends). Each opt-in file carries a block:

  ```rust
  #![warn(clippy::pedantic)]
  #![allow(clippy::module_name_repetitions)]   // load_*_overview is idiomatic
  #![allow(clippy::must_use_candidate)]        // every getter would trip it
  ```

  Additional `#[allow]`s are added sparingly and always on the specific
  item, never at the module level, to avoid hiding new warnings behind old
  blanket allows. Modules that would need more than ~10 per-item allows to
  go green stay out of pedantic for now and carry a `// TODO: opt into
  clippy::pedantic after plan/11 §9.1` note at the top.
- `clippy::unwrap_used` / `clippy::expect_used` are already covered by the
  rust-style rule file; they are applied in CI via `-W` flags rather than
  in source, so test code (which legitimately uses `.expect()` for
  assertion messages) is not penalised.
- `cargo-deny check` (§9.4) and `scripts/check-layers.sh` (§9.7) are the
  two "beyond clippy" guardrails: the first catches dependency-level
  drift, the second catches layer-boundary drift. Both run in CI
  (§9.6) and are optional but encouraged locally via the pre-commit
  hook documentation in §9.5.

### 9.2 `Clock` port coverage

- The `Clock` port in `src/application/ports/clock.rs` is the single
  abstraction over monotonic time inside the crate. No other place in
  `src/application/` or `src/domain/` calls `Instant::now()`,
  `SystemTime::now()` or `std::time::Instant::now()`.
- Adapters may call wall-clock / monotonic primitives directly when:
  - the value is not observable by downstream code (e.g. a unique suffix
    in a temp filename in `src/adapters/config/fs.rs`), or
  - the value is immediately folded into an HTTP request and the test
    asserts only on the stubbed response, not on the request timestamp
    (e.g. `current_unix_seconds()` in `src/adapters/prices/alchemy_prices.rs`).
  Both carry a `// NOTE: adapter-local wall clock; see plan/11 §9.2.`
  comment so reviewers can tell they were a deliberate choice.
- `tests/support/stubs.rs::FrozenClock` is the only `Clock` used in tests.
  Every functional test that depends on time (cache TTL, ENS reverse
  lookup TTL, search feed dedup) drives expiry via `FrozenClock::advance`.
  Adding new time-dependent behaviour without a `FrozenClock`-driven test
  is a review-blocker.

### 9.3 `Rng` port

- Port: `src/application/ports/rng.rs`.
  ```rust
  pub trait Rng: Send + Sync {
      fn fill_bytes(&self, dest: &mut [u8]);
      fn next_u64(&self) -> u64 { /* default impl via fill_bytes */ }
  }
  ```
  Interior-mutability based (`&self`) so an `Arc<dyn Rng>` can be shared
  across tasks without wrapping every stream in a `Mutex`.
- Production adapter: `src/adapters/rng.rs::OsRng` reads from
  `/dev/urandom` on Unix via `std::fs::File`; the file handle is
  opened lazily on first use and kept open for the process lifetime
  (the expected access pattern for `/dev/urandom`). No new top-level
  crate. Non-Unix targets panic with a pointer at this plan section
  until the fallback adapter lands.
- Stub: `tests/support/stubs.rs::SeededRng` wraps a deterministic
  xorshift64* generator (inline implementation, no new crate). Exposes
  `SeededRng::new(seed)` (non-zero seed required) and
  `SeededRng::default()` (seed `0xdead_beef_cafe_f00d`). Unit tests in
  `tests/functional/rng_stub.rs` assert that two `SeededRng::new(42)`
  instances produce the same stream.
- The port has no callers yet; it is introduced now so the first
  feature that needs randomness (future nonce generation, jitter on
  retry back-off, etc.) can pick it up without a second refactor.

### 9.4 `cargo-deny` configuration

- `deny.toml` at the repo root with four sections:
  - `[advisories]` — `version = 2` format; `yanked = "deny"`; `ignore` list
    empty today with a comment pointing at the review process.
  - `[licenses]` — `version = 2`; `allow = ["MIT", "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause",
    "ISC", "Unicode-3.0", "Unicode-DFS-2016", "Zlib", "Unlicense",
    "CC0-1.0", "MPL-2.0", "OpenSSL"]`. Anything else must be justified in
    an explicit `exceptions` entry citing the plan.
  - `[sources]` — only `crates.io` and the vendored github mirrors cargo
    resolves by default; `unknown-registry = "deny"`,
    `unknown-git = "deny"`.
  - `[bans]` — `multiple-versions = "warn"`; intentionally left as a
    warning because the dep graph already contains legitimate duplicate
    versions (e.g. `getrandom` 0.2/0.3/0.4) that we cannot force-unify.
- `cargo deny check` runs in CI on every push and PR via
  `taiki-e/install-action` with a pinned `cargo-deny` release so the
  advisory database (including CVSS 4.0 metadata) keeps parsing.

### 9.5 Pre-commit hook

- Lives at `.githooks/pre-commit` (a plain shell script; not under
  `.cursor/hooks/` because those are Cursor-specific hooks with a
  different contract).
- Installs via `scripts/install-hooks.sh`, which sets
  `git config core.hooksPath .githooks` inside the current clone. The
  script is idempotent and documented in the README.
- On commit the hook runs:
  1. `cargo fmt --all -- --check` over the whole crate (fast; rustfmt
     amortises well with the cached build graph).
  2. `cargo clippy --all-targets -- -D warnings` on the whole crate.
  Both are whole-crate rather than incremental because cargo's
  incremental cache already makes them fast and the correctness
  trade-off of running only on staged files (missing cross-file breakage)
  is not worth the complexity for a single-crate repo.
- The hook respects `--no-verify` for emergency escape hatches but logs
  a warning so the author notices at push time.

### 9.6 CI configuration

- File: `.github/workflows/ci.yml`. Triggers on `push` and
  `pull_request` to every branch.
- One job (`lint-and-test`) on `ubuntu-latest`, stable Rust pinned
  through `dtolnay/rust-toolchain@stable` with `rustfmt` and `clippy`
  components.
- Caching: `Swatinem/rust-cache@v2` keyed on `Cargo.lock`.
- Steps, in order:
  1. `cargo fmt --all -- --check`.
  2. `cargo clippy --all-targets -- -D warnings`.
  3. `cargo test --lib --bins` (unit tests).
  4. `cargo test --tests` (functional tests under `tests/functional`).
  5. `cargo test --test e2e` (cucumber scenarios).
  6. `cargo deny check`.
  7. `scripts/check-layers.sh` (see §9.7).
- **Network disabled for test steps.** Runners are granted full
  network during build / fetch, but steps 3–5 run under
  `bash scripts/ci-run-no-network.sh cargo test …` (wraps `sudo env …
  unshare -n` with `PATH`, `HOME`, `RUSTUP_HOME`, `CARGO_HOME` forwarded
  so rustup does not try to populate `/root/.rustup` inside the netns).
  `sudo unshare -n` creates a network namespace only (no `-r` user
  namespace remap — GitHub-hosted runners deny `uid_map` writes for
  `unshare -rn`). There is no route to the internet, and `lo` starts
  **DOWN** until `ip link set lo up`, which keeps `wiremock` on
  `127.0.0.1` working.
  Any accidental real HTTP call to a non-loopback address fails with
  `ENETUNREACH`, which is the loud failure mode `.cursor/rules/testing.mdc`
  requires. When
  `unshare` is not available (e.g. hosted-runner policy changes) the
  workflow falls back to the `NO_NETWORK=1` env guard read by the test
  harness; that fallback is documented in the README.
- `cargo fetch` is executed before network is dropped so the
  dependency cache is warm.
- Tests remain runnable locally with plain `cargo test` — the
  network-disabled wrapper is CI-only.

### 9.7 Layer-import enforcement

- Script: `scripts/check-layers.sh`. Pure `grep`-based, no extra
  tooling needed.
- Rules enforced (from `.cursor/rules/architecture.mdc`):
  - `src/domain/**/*.rs` must not import `crate::application` or
    `crate::adapters` or `crate::infra`.
  - `src/application/**/*.rs` must not import `crate::adapters` or
    `crate::infra`.
  - `src/adapters/ui/**/*.rs` must not import `crate::adapters::{rpc,
    etherscan, signatures, ens, cache, prices, labels, secrets,
    config}`. UI depends only on application ports.
- Exit code non-zero on any violation, with file and line printed.
- `cargo modules` is deferred to a follow-up because installing it as a
  CI step adds a five-minute compile tax for a marginal gain over the
  grep script. A README note records the trade-off and a TODO entry
  keeps the option open if the grep script proves too coarse.
