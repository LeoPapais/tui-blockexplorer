# 10 — Settings

Status: **done** (MVP scope) — read-only Settings screen is live
behind `s` on Home; both BDD scenarios in
`tests/e2e/features/settings.feature` are green. Editing
credentials / chains / theme / keybinds / cache all remain deferred
(section 11.2).

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

- Should we encrypt the config file? Out of scope for MVP; recommend using the env
  variable when on a shared machine.
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
