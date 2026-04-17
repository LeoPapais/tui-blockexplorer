# 9 — Gas Tracker

Status: **done** (MVP scope) — `g` on Home opens a Gas Tracker
screen that renders `slow / average / fast / base fee / trend` from
the shared `GasOraclePort`. Live mode polls Alchemy every 6s via a
dedicated feed task. The unit converter modal, percentile histogram
and pending base-fee prediction remain deferred (section 11.2).

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

### 11.2 Deferred

- Unit converter modal (`u`).
- Base-fee prediction for the pending block.
- Percentile histogram widget.
