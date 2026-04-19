# 10 — Settings

Status: **partial** — read-only Settings screen is live behind `s` on
Home; seven of the eight §11.2 follow-ups now have port / adapter /
BDD coverage (April 2026, see section 12). Editing credentials,
chains, theme and keybinds at runtime remains explicitly WONT-DO for
MVP.

Configuration UI. Key management, chain registry, theme, keybinds, cache, secondary
providers. Reads and writes `~/.config/blockexplorer-tui/config.toml`.

## 1. Purpose and user goals

- Set credentials for Alchemy and Etherscan.
- Enable and disable chains (drives the chain picker on Home).
- Change the active chain persistently.
- Customise theme and keybindings.
- Inspect cache TTL and reset the cache.

## 2. Layout

```
+-- Breadcrumb ----------------------------------------------------+
| Home > Settings                                                  |
+-- Sections ------------------------------------------------------+
| [Credentials]  Chains  Theme  Keybinds  Cache  About             |
+-- Content -------------------------------------------------------+
| Alchemy API key      [set]   (status: valid)                     |
| Etherscan API key    [set]   (status: not set)                   |
| Openchain API key    [set]   (status: optional)                  |
+-- Status bar ----------------------------------------------------+
| s save  r reset section  Ctrl+T test credentials                 |
+------------------------------------------------------------------+
```

Sections:

- **Credentials**: Alchemy, Etherscan, optional Openchain.
- **Chains**: list of supported chains with an enabled/disabled toggle and a default
  chain radio.
- **Theme**: dark (default), light, custom palette (color picker modal).
- **Keybinds**: list of actions with the current binding; `Enter` on a row opens a
  "press new key" modal; conflicts are detected.
- **Cache**: show TTL per adapter, current size, `Ctrl+X` to clear.
- **About**: version, commit sha, link to `plan/README.md`.

## 3. Keybindings

| Key       | Action                                    |
|-----------|-------------------------------------------|
| `Tab`     | Next section                              |
| `s`       | Save changes                              |
| `r`       | Reset changes in the current section      |
| `Ctrl+T`  | Test provider credentials                 |
| `Ctrl+X`  | Clear cache (Cache section only)          |

## 4. Use cases

### 4.1 `LoadConfig`

- **Input**: none.
- **Output**: current `AppConfig`.
- **Ports**: `ConfigPort::load`.
- **Behaviour**: reads the file; missing file returns a default config flagged as
  "first run".

### 4.2 `UpdateConfig`

- **Input**: a partial `ConfigPatch`.
- **Output**: updated `AppConfig`.
- **Ports**: `ConfigPort::save`.
- **Behaviour**: validates the patch; atomically writes the file; returns the new
  config so live streams can be restarted.

### 4.3 `ValidateProviderCredentials`

- **Input**: `Provider` and `ApiCredentials`.
- **Output**: `CredentialStatus { valid, error: Option<String> }`.
- **Ports**: per-provider `HealthPort::ping` (lightweight endpoint: Alchemy
  `eth_chainId`, Etherscan `stats`).

## 5. Ports required

- `ConfigPort`: `load`, `save`.
- `SecretStorePort` (optional future): OS keychain integration. MVP keeps keys in
  the config file or env.
- `HealthPort` per adapter.

## 6. Data sources

- Local file `~/.config/blockexplorer-tui/config.toml`.
- Env overrides: `ALCHEMY_API_KEY`, `ETHERSCAN_API_KEY`, `OPENCHAIN_API_KEY`.
- Health pings: Alchemy `eth_chainId`, Etherscan V2 `stats`.

## 7. BDD scenarios (`tests/e2e/features/settings.feature`)

```gherkin
Feature: Settings

  Scenario: First run prompts for Alchemy key
    Given there is no config file and no env variables are set
    When the app starts
    Then the Settings screen opens on the Credentials section
    And a banner says "Set your Alchemy API key to get started"

  Scenario: Invalid Alchemy key shows error
    Given the user enters an Alchemy key
    And Ctrl+T is pressed
    And the stub returns an auth error
    Then the status for Alchemy shows "invalid: 401"

  Scenario: Changing default chain updates all screens
    Given the user is on Settings > Chains
    When the user changes the default chain to "base"
    And presses "s"
    Then the config on disk reflects "base" as the default chain
    And reopening the app launches on "base"

  Scenario: Keybind conflict is rejected
    Given the user assigns "Enter" to "Copy Address" on the Keybinds section
    Then an error modal says "Enter is already bound to Open Item"
    And the binding is not saved
```

## 8. Functional tests

- `LoadConfig`: missing file -> default with `first_run=true`; malformed file ->
  `DomainError::Config`; partial file merged with defaults.
- `UpdateConfig`: atomic write (temp file + rename); refuses to save when disk is
  full (error is surfaced).
- `ValidateProviderCredentials`: Alchemy valid / invalid; Etherscan valid / invalid;
  network error is distinct from auth error.

## 9. Fixtures

- `config__default.toml`
- `config__first_run_missing.toml` (synthetic; represents no file)
- `config__malformed.toml`
- `rpc__eth_chainId__ok.json`
- `rpc__eth_chainId__auth_error.json`
- `etherscan__stats__ok.json`
- `etherscan__stats__auth_error.json`

## 10. Open questions

- Should we encrypt the config file? **Resolved (April 2026).** The
  MVP ships a `SecretStorePort` trait with a single
  `PlaintextSecretStore` adapter that is explicitly logged as
  insecure at startup and delegates to the TOML config file.
  OS-keychain integration (`keyring` crate) stays deferred because
  the added dependency surface is not justified while
  `ALCHEMY_API_KEY` is the only secret that must be persisted. See
  section 12.5 for the decision log.
- Should we support multiple Alchemy apps per chain? No in MVP; one app covers every
  chain via the Alchemy multichain endpoint.

## 11. Implementation plan

Single slice — the config loader already exists (`plan/14`). This slice
only adds a read-only screen and a Home keybinding.

### 11.1 Slice — UI + Home binding + BDD

- `src/adapters/ui/settings.rs` — `SettingsScreen` that receives an
  `AppConfigSnapshot` (simple view-model) and renders:
  - active chain slug,
  - Alchemy key presence ("configured" | "not set"),
  - config-file path hint,
  - explicit note listing what editing remains deferred.
- Home gains an optional `settings_factory` bound to the `s` key.
- `infra::run` passes a snapshot of the loaded `AppConfig` into the
  factory so the screen shows the same values the runtime booted
  with.
- BDD `tests/e2e/features/settings.feature`: one scenario where
  pressing `s` from Home shows the key status as "configured" once
  the world has primed a fake key.

### 11.2 Deferred

- Editing credentials / chains / theme / keybinds (requires
  persistent write + validation pipeline).
- Ctrl+T "test credentials" round-trip.
- Cache size display and reset.

## 12. §11.2 follow-ups — plan-first entry (April 2026)

This section resolves the seven non-WONT-DO §11.2 items enumerated
in `plan/15-backlog.md` §8.11 (+§8.15 for `ConfigPort::save`). The
recommended order is cheap → expensive: logger masking first so
nothing else leaks while we work, then the visual first-run banner,
then the persistent primitives (`ConfigPort::save`, keybind
conflicts, `HealthPort`, `SecretStorePort`), and finally the
palette-preset data model.

### 12.1 Logger masking (§11.2 item 7)

- New module `src/infra/logging.rs` exposing `install_masking_logger()`.
  Builds a `tracing_subscriber::fmt` layer whose writer runs every
  rendered log line through a pure helper
  `mask_sensitive(&str) -> String`. The helper redacts the value of
  any field whose key contains `key`, `token` or `secret`
  (case-insensitive) by the exact replacement `<redacted>`. The
  helper recognises both structured field syntax
  (`alchemy_key=abc`, `api-token="..."`) and JSON-like pairs
  (`"secret": "abc"`).
- Unit tests under `tests/functional/logger_masking.rs` exercise the
  helper directly on representative lines and end-to-end through
  an in-process `MakeWriter` backed by `Arc<Mutex<Vec<u8>>>`. See
  `.cursor/rules/external-apis.mdc` §"Credentials".
- `infra::run` calls `install_masking_logger()` before anything else.
  The logger defaults to `ERROR` on stderr and is a no-op when a
  global subscriber is already installed (avoids poisoning tests).

### 12.2 First-run banner (§11.2 item 6)

- `HomeScreen` gains a `first_run_hint: bool` flag set by `infra`
  when `!config.has_alchemy_key() && !cli.demo`. In practice the
  binary still exits early in that case, but when invoked with the
  demo flag against a missing key it now shows a dismissible banner
  at the top (one text line plus a `Tab / s / Esc` hint). The banner
  is also surfaced on the Settings screen (new line at the top of
  the Overview body).
- Dismissal: `Esc` or `Enter` on Home. Pressing `s` opens Settings as
  before.
- Snapshot: `tests/functional/home_screen_first_run_banner.rs` uses
  `TestBackend` to assert the banner appears and disappears on Esc.
  BDD: a new scenario in `tests/e2e/features/home.feature` —
  `Scenario: First-run banner routes the user to Settings`.

### 12.3 `ConfigPort::save` atomic write (§11.2 item 3 / §8.15)

- New port `ConfigPort` under `src/application/ports/config.rs` with
  `load(&self) -> Result<AppConfigView, DomainError>` and
  `save(&self, patch: &ConfigPatch) -> Result<(), DomainError>`.
  `AppConfigView` is a read-only copy exposed to the application
  layer; `ConfigPatch` starts with a single field
  (`default_chain: Option<Chain>`) — future editing slices extend it.
- Adapter `FsConfig` under `src/adapters/config/fs.rs` implementing
  both methods. `save` resolves the target path (either the one
  passed to the constructor or the XDG default), ensures the parent
  directory exists, writes to `<path>.tmp.<pid>-<ts>`, calls
  `File::sync_all`, and renames into place. Errors map into
  `DomainError::Config` with a clear message.
- Functional tests `tests/functional/fs_config_save.rs` use
  `CARGO_TARGET_TMPDIR` (mirrors the pattern already used by
  `tests/functional/config_load.rs`). Happy: saved TOML round-trips
  via `ConfigLoader`. Failure: a non-writable directory surfaces
  `DomainError::Config`. A second test proves the `.tmp` is removed
  on success and that the target file only appears after the rename
  (by creating a sentinel and checking atomicity semantics via
  directory listing).
- BDD: `Scenario: Persist default chain change` in
  `tests/e2e/features/settings.feature`. The scenario uses an
  in-memory stub `ConfigPort` rather than touching disk; the
  adapter-level atomicity sits in the functional test.

### 12.4 Keybind conflict detection modal (§11.2 item 1)

- Introduce `Action` enum (subset of today's key handlers:
  `Quit`, `Back`, `OpenSearch`, `OpenMempool`, `OpenGasTracker`,
  `OpenSettings`). Placed under
  `src/domain/key_binding.rs` as pure types (no I/O).
- `KeyMap` is a `HashMap<(ScreenId, KeyEvent), Action>` wrapper with
  a constructor `KeyMap::builtin()` (the defaults currently hard-coded
  on `HomeScreen`) and a `KeyMap::from_entries(entries)` that returns
  `Result<KeyMap, KeyBindConflict>` — `KeyBindConflict` lists the
  offending `(ScreenId, KeyEvent)` and the actions it is mapped to.
- New screen `KeyBindConflictModal` under
  `src/adapters/ui/keybind_modal.rs` displays the conflict list and
  is dismissed with `Esc`. `infra::run` pushes this modal on top of
  Home when `KeyMap::from_entries` fails on user-supplied overrides.
  For MVP we never accept overrides yet so the failure path is
  exercised via tests only; the modal exists so the UX is ready when
  overrides land.
- Tests: `tests/functional/keymap_conflict.rs` (happy: no conflict,
  failure: duplicate binding → `KeyBindConflict` with both actions),
  and a snapshot of the modal rendering.

### 12.5 `HealthPort` (§11.2 item 4)

- Port `HealthPort` under `src/application/ports/health.rs` returning
  `HealthStatus { provider: &'static str, status: HealthLevel, latency_ms: u64, message: Option<String> }` where
  `HealthLevel::{Healthy, Degraded, Down}`. Two methods:
  `alchemy(&self, chain)` and `etherscan(&self, chain)`.
- Adapters:
  - `AlchemyHealth` under `src/adapters/rpc/health.rs`: calls
    `eth_chainId` against the active chain URL; `Healthy` on match,
    `Degraded` on mismatch, `Down` on `RpcError`.
  - `EtherscanHealth` under `src/adapters/etherscan/health.rs`: calls
    `module=stats&action=chainsize&chainid={id}` (lightweight,
    available on every chain Etherscan V2 indexes). `Healthy` on
    `status=="1"`, `Degraded` on a `200 OK` with `status=="0"`,
    `Down` on network error.
- Functional tests via `wiremock` for both adapters — happy +
  timeout + 429. Fixtures added under `tests/fixtures/` (JSON
  bodies only, no secrets).
- Settings screen (`src/adapters/ui/settings.rs`) grows a second
  section "Providers" rendered below the Overview body, showing
  each provider's `HealthLevel` and latency. The data comes from a
  single snapshot taken on screen open; live polling is deferred.

### 12.6 `SecretStorePort` (§11.2 item 5)

- Port `SecretStorePort` under
  `src/application/ports/secret_store.rs` with:
  `get(&self, key: &str) -> Result<Option<String>, DomainError>` and
  `set(&self, key: &str, value: &str) -> Result<(), DomainError>`.
- Adapter `PlaintextSecretStore` under
  `src/adapters/secrets/plaintext.rs` backed by an
  `Arc<RwLock<HashMap<String, String>>>` persisted through
  `ConfigPort::save` on every `set`. Explicitly logs a warning at
  construction: *"SecretStore: plaintext backend in use, keys are
  stored unencrypted in config.toml"*. The warning runs through the
  masking logger from §12.1 so the value is never echoed.
- OS-keychain integration (e.g. via `keyring`) stays **deferred**.
  The trait is small enough that swapping the adapter is a drop-in
  change once we pick a keychain crate; see §10 open questions.
- Tests: `tests/functional/plaintext_secret_store.rs` — round-trip
  through an in-memory stub `ConfigPort` (no real filesystem).

### 12.7 Custom palette presets (§11.2 item 2)

- `src/adapters/ui/theme.rs` introduces a `Palette` struct with
  semantic tokens (`accent`, `warning`, `success`, `muted`) and a
  `PalettePreset` enum
  (`DarkDefault`, `Light`, `HighContrast`, `Solarized`). The interactive
  RGB picker stays deferred — switching happens via
  `Settings → Theme` section by pressing `1..N`.
- Widgets that used raw `ratatui::style::Color` are migrated in a
  separate slice; this plan only ships the data model + the preset
  list + the Settings UI. Feature-flagged behind a `theme` column in
  the Settings overview body.
- BDD scenario: *Scenario: switch to high-contrast palette applies
  semantic tokens immediately* — exercised with a `TestBackend`
  snapshot; full UI migration is explicitly a follow-up.

### 12.8 Decisions resolved by this slice

- **§10 OQ1 (encrypt config file)** — resolved as documented above.
- **§8.15 `ConfigPort::save`** — implemented here; `plan/14`
  section 7 now references §12.3 as the canonical location.
- **§8.11 "Env overrides for `ETHERSCAN_API_KEY` and
  `OPENCHAIN_API_KEY`"** stays WONT-DO: the current adapter
  boundary accepts `ApiCredentials` from config + env already for
  Alchemy, and Etherscan reads from the config file path the
  adapter constructs. Widening env override to a third variable
  adds surface without demand.

### 12.9 Still deferred after this slice

- Interactive RGB picker (§12.7 picks presets only).
- Live `HealthPort` polling: today's Settings screen takes a single
  snapshot on open; a refresh loop is a small follow-up.
- OS-keychain adapter (see §12.6).
- Loading user-supplied keybind overrides from `config.toml`: the
  `KeyMap` infrastructure ships, but the `[keybinds]` config
  section stays unread.
- Full theme migration of every widget to semantic tokens.
