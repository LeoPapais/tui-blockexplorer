# 9 — Gas Tracker

Status: **done** (MVP + follow-ups) — `g` on Home opens a Gas Tracker
screen that renders `slow / average / fast / base fee / trend` from
the shared `GasOraclePort`. Live mode polls Alchemy every 6s via a
dedicated feed task. Follow-ups §11.2 (pause / Ctrl+R), §11.3
(percentile histogram) and §11.4 (unit converter modal) shipped in
the §8.10 backlog slice. Pending base-fee prediction stays WONT-DO
(§11.5).

Full-screen gas dashboard. Reached from Home (`Enter` on the Gas card) or via the
command palette. Shares the `GasOraclePort` with Home.

## 1. Purpose and user goals

- Answer "what gas price should I use right now?" with confidence.
- Show base fee trend and priority fee distribution.
- Provide a unit converter modal (wei / gwei / ether).

## 2. Layout

```
+-- Breadcrumb ----------------------------------------------------+
| Home > Gas Tracker                                               |
+-- Speed cards ---------------------------------------------------+
| +-- Slow ------+  +-- Average ---+  +-- Fast ------+             |
| | 12 gwei      |  | 14 gwei      |  | 18 gwei      |             |
| | ~45s         |  | ~20s         |  | ~10s         |             |
| +--------------+  +--------------+  +--------------+             |
+-- Base fee trend (last 20 blocks) -------------------------------+
| 11.4 gwei                                                        |
|  . : = ~ - _ . : = ~                                             |
+-- Priority fee distribution (p25 / p50 / p75) -------------------+
| p25 0.10   p50 0.20   p75 0.50  gwei                             |
| mini-histogram                                                   |
+-- Status bar ----------------------------------------------------+
| u unit converter  p pause  Ctrl+R refresh                        |
+------------------------------------------------------------------+
```

## 3. Keybindings

| Key      | Action                              |
|----------|-------------------------------------|
| `u`      | Open unit converter modal           |
| `p`      | Pause / resume updates              |
| `Ctrl+R` | Force a manual recompute            |

## 4. Use cases

### 4.1 `ObserveGasOracle`

Shared with Home. See `1-home.md` section 4.2.

### 4.2 `ConvertUnits`

- **Input**: amount + source unit (`Wei`, `Gwei`, `Ether`) + target unit.
- **Output**: target-unit amount as a string, plus the exact integer representation
  in wei.
- **Pure domain function**; no ports; lives in `domain/gas.rs`.
- **Behaviour**: overflows return `DomainError::Overflow`; negatives are rejected.

## 5. Ports required

- `GasOraclePort` (shared).

## 6. Data sources

- Alchemy: `eth_gasPrice`, `eth_maxPriorityFeePerGas`, `eth_feeHistory` with
  `rewardPercentiles=[25, 50, 75]` over the last 20 blocks. Stream is driven by the
  `newHeads` subscription.

## 7. BDD scenarios (`tests/e2e/features/gas_tracker.feature`)

```gherkin
Feature: Gas tracker

  Background:
    Given the active chain is "ethereum"

  Scenario: Cards update each new block
    Given the user is on Gas Tracker
    When the stub emits a new head with higher base fee
    Then the Slow, Average and Fast cards update their gwei values
    And the base fee trend sparkline shifts left

  Scenario: Fee history window renders
    Given the stub returns 20 blocks of fee history
    When the user is on Gas Tracker
    Then the trend has 20 sample points

  Scenario: Unit converter handles ETH input
    Given the user is on Gas Tracker
    When the user opens the unit converter
    And enters "1" with unit "ether" and target "wei"
    Then the result is "1000000000000000000"

  Scenario: Unit converter rejects negatives
    When the user enters "-1" into the converter
    Then an inline error "negative values not allowed" is shown
    And the result field is empty
```

## 8. Functional tests

- `ObserveGasOracle`: happy path with 20 blocks of fee history; empty reward array
  handled gracefully.
- `ConvertUnits`: parametrized over (source, target, amount, expected). Edge cases:
  zero, max u128, negative -> error, fractional ether -> precise wei.

## 9. Fixtures

- `rpc__eth_feeHistory__ethereum_20_blocks.json`
- `rpc__eth_feeHistory__empty_rewards.json`
- `rpc__eth_gasPrice__ethereum.json`
- `rpc__eth_maxPriorityFeePerGas__ethereum.json`
- `rpc__newHeads__ethereum_sequence_5.json`

## 10. Open questions

- Should we save the last N fee samples across sessions? No in MVP; it is cheap to
  recompute.
- Should we show the pending base fee prediction for the next block (EIP-1559
  formula)? Yes, computed locally from the latest base fee and gas usage ratio.

## 11. Implementation plan

One slice — the port and domain already exist (shared with Home).

### 11.1 Slice — UI + feed + BDD

- `src/adapters/ui/gas_tracker.rs` — `GasTrackerScreen` with a feed
  channel, three speed cards, a base-fee trend line and a raw-gwei
  readout. Deferred UI: unit converter modal, percentile histogram.
- `src/infra/gas_feed.rs` — `spawn` helper that owns a
  `GasOraclePort`, polls at the same 6s interval the Home session
  uses, and publishes `GasSnapshot` updates into the screen's
  channel.
- Home gains an optional `gas_factory` bound to the `g` key that
  pushes a fresh `GasTrackerScreen`.
- BDD `tests/e2e/features/gas_tracker.feature`:
  - From Home, press `g` and assert a "Gas Tracker" screen is on
    top with the primed snapshot's slow/average/fast values.
  - After the stub publishes a new snapshot, assert the screen
    updates.

### 11.2 Shipped — Pause + manual refresh (`p` / Ctrl+R)

Promoted out of `plan/15-backlog.md` §8.10.

- `GasTrackerScreen` gained a `paused: bool` flag. `p` toggles it.
- While paused, `tick` keeps draining the feed channel (so the
  unbounded mpsc cannot back-pressure the feed task) but **drops**
  every incoming `GasSnapshot` instead of replacing `current`.
  The "paused" badge in the header shows the state.
- `Ctrl+R` returns `Command::Refresh` **and**, when the screen was
  built with a refresh handle, kicks a `GasRefreshHandle` that is
  consumed by the feed task via `tokio::select!` so the next
  `oracle.snapshot` call happens immediately instead of waiting
  for the 6s period. A manual refresh is honoured even while the
  screen is paused, with the explicit semantics "unpauses + applies
  the next snapshot once". The badge then returns to "live".
- The feed task (`src/infra/gas_feed.rs::spawn`) takes an optional
  `GasRefreshListener`; the composition root wires one end into the
  screen and the other into the spawn call. Tests that don't need a
  refresh path keep the old 2-arg `gas_feed()` helper.

### 11.3 Shipped — Percentile histogram (p25 / p50 / p75)

- New pure helper `Gwei::percentiles(samples: &[Gwei]) ->
  Percentiles { p25, p50, p75 }` in `src/domain/gas.rs`. Nearest-rank
  method (same definition the RPC layer already uses via
  `eth_feeHistory`'s `rewardPercentiles`). Empty input returns
  `Percentiles::empty()` so the widget can render a neutral state.
- The Gas Tracker screen keeps a rolling buffer of the last 60
  snapshots (one per polling tick). The histogram block renders
  `p25 / p50 / p75` of `base_fee` alongside the tier numbers, plus a
  tiny horizontal bar for each percentile using Unicode block
  characters. Two ASCII fallback characters keep the widget readable
  on terminals without a block-element font.

### 11.4 Shipped — Unit converter modal (`u`)

- New domain helper `gas::convert_unit(value: &str, from: Unit, to:
  Unit) -> Result<String, DomainError>` with the `Unit` enum
  (`Wei / Gwei / Ether`). Validation rules:
  - Reject strings that start with `-` → `InvalidInput("negative
    values not allowed")`.
  - Parse the input as a fixed-point decimal with up to **18**
    fractional digits (ether max precision). Values with more digits
    or with invalid characters return `InvalidInput`.
  - Any intermediate overflow of the internal `u128` representation
    surfaces as `InvalidInput("value exceeds u128 range")`.
  - The output is a canonical decimal string: integer values have no
    trailing `.` / `0`; fractional values are trimmed of trailing
    zeros.
- The Gas Tracker screen owns a lightweight modal struct kept
  strictly internal to the file (no cross-screen modal runtime yet —
  `plan/15-backlog.md` §8.13 tracks that). `u` opens it, `Esc`
  closes it, `Enter` performs the conversion, arrow keys flip the
  source / target units. Errors are rendered inline under the input
  field. The modal is rendered by the screen's own `render`, centred
  over the main content.
- Follow-up: migrate the modal to `Command::OpenModal` once the
  dispatcher gains it in §8.13.

### 11.5 Deferred

- Base-fee prediction for the pending block (EIP-1559 formula). This
  is pure math but the UX cost — another column + error bars —
  outweighs the benefit today. Stays tracked in `plan/15-backlog.md`
  §8.10 as WONT-DO.
