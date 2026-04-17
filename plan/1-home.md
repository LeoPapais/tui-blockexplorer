# 1 — Home

Landing screen. Only two content sections: **Network** and **Gas Tracker**. The chain
switcher lives in the header of this screen. There are no live feeds of blocks or
transactions here (explicitly removed from scope).

## 1. Purpose and user goals

- Give the user a live, at-a-glance pulse of the chain they are on.
- Let the user change chain in two keystrokes (`c`, pick).
- Serve as a landing pad: from here users open Settings, Mempool, Gas Tracker, and
  trigger Universal Search.

## 2. Layout

```
+-- Header --------------------------------------------------+
| [Chain: Ethereum v]       ? help  /  search  :  cmd        |
+-- Content -------------------------------------------------+
|                                                            |
|  +-- Network --------------+  +-- Gas Tracker -----------+ |
|  | Latest block   21345678 |  | Slow    12 gwei  ~45s    | |
|  | Block time avg    12.1s |  | Avg     14 gwei  ~20s    | |
|  | Base fee      11.4 gwei |  | Fast    18 gwei  ~10s    | |
|  | TPS (last 20)     15.3  |  | Base fee   11.4 gwei     | |
|  | Peers connected     OK  |  | Trend  . , - = ~ ~ _ =   | |
|  | Latency           45 ms |  |                          | |
|  +-------------------------+  +--------------------------+ |
|                                                            |
+-- Status bar ----------------------------------------------+
| c chain  Enter gas details  gm mempool  gs settings        |
+------------------------------------------------------------+
```

The two cards render side-by-side on wide terminals (>= 120 columns) and stack
vertically on narrow ones.

## 3. Keybindings (screen-specific)

| Key      | Action                                         |
|----------|------------------------------------------------|
| `c`      | Open chain picker modal                        |
| `Enter`  | Open full `GasTracker` screen                  |
| `gm`     | Go to Mempool                                  |
| `gs`     | Go to Settings                                 |
| `Ctrl+R` | Force refresh (resubscribes `newHeads`)        |

Global bindings from `0-general-architecture.md` also apply.

## 4. Use cases

### 4.1 `ObserveNetworkStatus`

- **Input**: current `Chain`.
- **Output**: a stream of `NetworkStatus { latest_block, block_time_avg_ms, base_fee,
  tps, latency_ms }`.
- **Ports used**: `NetworkStatusPort`.
- **Behaviour**: opens a `newHeads` subscription. On each head, recomputes block time
  average over the last N=20 blocks, computes TPS, reports latency from WS ping.

### 4.2 `ObserveGasOracle`

- **Input**: current `Chain`.
- **Output**: a stream of `GasSnapshot { slow, average, fast, base_fee,
  priority_percentiles, sparkline }`.
- **Ports used**: `GasOraclePort` (shared with `9-gas-tracker.md`).
- **Behaviour**: on each new head, calls `eth_feeHistory` with
  `rewardPercentiles=[25,50,75]` over the last 20 blocks and emits a new snapshot.

### 4.3 `SwitchChain`

- **Input**: target `Chain`.
- **Output**: `Result<(), DomainError>`.
- **Ports used**: `ChainRegistryPort`, `ConfigPort`.
- **Behaviour**: validates the chain is enabled, tears down existing subscriptions,
  clears per-chain cache, updates the active chain in application state, restarts
  `ObserveNetworkStatus` and `ObserveGasOracle`.

## 5. Ports required

- `NetworkStatusPort` — `async fn subscribe(chain) -> Stream<NetworkStatus>`.
- `GasOraclePort` — `async fn subscribe(chain) -> Stream<GasSnapshot>` and
  `async fn snapshot(chain) -> GasSnapshot`.
- `ChainRegistryPort` — `fn list_enabled() -> Vec<Chain>` and
  `fn details(chain) -> ChainMetadata`.
- `ConfigPort` — `fn active_chain() -> Chain` and `fn set_active_chain(chain)`.

Implementations live under `src/adapters/rpc/` (Alchemy) and `src/adapters/config/`.

## 6. Data sources

All via Alchemy:

- `eth_blockNumber`
- `eth_getBlockByNumber`
- `eth_gasPrice`
- `eth_maxPriorityFeePerGas`
- `eth_feeHistory`
- `eth_subscribe("newHeads")` (WebSocket)

## 7. BDD scenarios (`tests/e2e/features/home.feature`)

```gherkin
Feature: Home screen

  Background:
    Given the user launches the app
    And the active chain is "ethereum"

  Scenario: User sees current network stats
    When the Home screen is rendered
    Then the Network card shows the latest block number from the stub
    And the Gas Tracker card shows slow, average and fast gwei values

  Scenario: Gas oracle updates on new block
    Given the Home screen is rendered
    When a new "newHeads" event is pushed from the stub
    Then the Network card updates the latest block number
    And the Gas Tracker card recomputes its values

  Scenario: User switches chain and stats refresh
    Given the Home screen is rendered with "ethereum"
    When the user presses "c"
    And selects "base"
    Then the active chain becomes "base"
    And the Network card reflects the latest block number for "base"

  Scenario: Connection drop shows degraded state
    Given the Home screen is rendered
    When the "newHeads" subscription drops
    Then the header shows a "disconnected" badge
    And the app schedules a reconnect
```

## 8. Functional tests

- `ObserveNetworkStatus`: happy path with 3 sequential heads; TPS math; fallback when
  first head has no parent to compute block time.
- `ObserveGasOracle`: happy path; graceful degradation when `eth_feeHistory` is
  unavailable on the current chain.
- `SwitchChain`: successful switch; switching to a disabled chain returns
  `DomainError::ChainNotEnabled`; switch mid-stream cancels prior subscriptions.

## 9. Fixtures

Under `tests/fixtures/` (naming: `{adapter}__{method}__{case}.json`):

- `rpc__eth_blockNumber__ethereum_mainnet.json`
- `rpc__eth_getBlockByNumber__ethereum_mainnet_latest.json`
- `rpc__eth_gasPrice__ethereum_mainnet.json`
- `rpc__eth_maxPriorityFeePerGas__ethereum_mainnet.json`
- `rpc__eth_feeHistory__ethereum_mainnet_20_blocks.json`
- `rpc__newHeads__ethereum_mainnet_sequence.json` (array of head events)
- `rpc__eth_blockNumber__base_mainnet.json`

## 10. Open questions

- How often do we sample the gas oracle when blocks are slow (for example chains with
  >10s block time)? Default: on every head; no extra timer.
- Do we show USD equivalent for fees in the header? Out of scope for MVP.
