# 1 — Home

Status: **done** — the 4 BDD scenarios in `tests/e2e/features/home.feature`
and the functional tests under `tests/functional/` are green.

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
vertically on narrow ones. The narrow-terminal fallback is snapshot-tested
under `tests/functional/home_screen_render.rs` (see §11.6).

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

  Scenario: Connection drop keeps last-known snapshots visible
    Given the Home screen is rendered
    When the "newHeads" subscription drops
    Then the header shows a "reconnecting" hint
    And the Network card still renders the last-known latest block
    And the Gas Tracker card still renders the last-known slow / average / fast gwei
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

## 11. Implementation plan

Concrete micro-steps followed to take this screen from red to green, ordered as
executed. Every bullet maps to a concrete module or file so the plan and the
tree remain aligned.

### 11.1 Domain types

New files under `src/domain/`:

- `chain.rs` — `pub enum Chain` with variants `Ethereum`, `EthereumSepolia`,
  `Base`, `Polygon`, `Optimism`, `Arbitrum`; `Chain::slug`, `Chain::from_slug`
  (returns `DomainError::InvalidInput` for unknown slugs), `Chain::all_enabled`.
- `block.rs` — `pub struct BlockNumber(u64)` newtype.
- `gas.rs` — `pub struct Wei(u128)` and `pub struct Gwei(u128)` newtypes with
  safe conversions (`to_gwei`, `to_wei`) and an `Amount::divide` helper (used
  by the gas oracle to fall back to integer math).
- `network_status.rs` — entities `NetworkStatus { chain, latest_block,
  base_fee_wei, block_time_avg_ms }` and `GasSnapshot { chain, slow_gwei,
  average_gwei, fast_gwei, base_fee_gwei, trend_gwei: Vec<Gwei> }`.

All types are `Clone + Debug + PartialEq + Eq` where sensible. They do not
derive serde traits in the domain module; the adapters own the
serialisation types and convert at the boundary.

### 11.2 Ports

New files under `src/application/ports/`:

- `network_status.rs` — `pub trait NetworkStatusPort` with a single
  `async fn snapshot(&self, chain: Chain) -> Result<NetworkStatus,
  DomainError>`.
- `gas_oracle.rs` — `pub trait GasOraclePort` with
  `async fn snapshot(&self, chain: Chain) -> Result<GasSnapshot,
  DomainError>`.
- `chain_registry.rs` — `pub trait ChainRegistryPort` (sync because it only
  reads config) with `list_enabled()`, `default_chain()` and
  `ensure_enabled(chain)`.

Traits use native `async fn` in traits. Implementations are consumed through
generics in the session code to avoid dyn-compat gotchas while the trait
surfaces are still small.

### 11.3 Home session (application layer)

A single coordinator composes the three use cases into one screen-facing
object:

- `src/application/home.rs`
  - `pub enum ConnectionStatus { Connected, Disconnected { reconnect_scheduled: bool } }`
  - `pub struct HomeViewModel { chain, network: Option<NetworkStatus>,
    gas: Option<GasSnapshot>, connection }`
  - `pub struct HomeSession<N, G, C>` owning the three ports and a
    `HomeViewModel`.
  - Methods: `new(network, gas, chains, chain)`, `refresh`, `on_new_head`,
    `on_connection_drop`, `switch_chain(target)`, `view`.

The three atomic "use cases" named in section 4 map to methods on the ports
plus the composition done by `HomeSession::refresh`. They stay trivial: the
value is in the coordination.

### 11.4 Stubs and fixtures

- `tests/fixtures/home__network_status__ethereum.json`,
  `tests/fixtures/home__gas_snapshot__ethereum.json`, plus `__base` variants.
  Each file holds one domain value deserialised by the stub with serde
  adapters declared in `tests/support/stubs.rs`.
- `tests/support/stubs.rs` gains three stubs:
  - `StubNetworkStatusPort` with `set_snapshot(chain, status)`,
    `set_broken(bool)`, and an `Arc<StubNetworkStatusPort>` blanket impl of
    `NetworkStatusPort`.
  - `StubGasOraclePort` mirroring the above for `GasSnapshot`.
  - `StubChainRegistry` holding a fixed list of enabled chains.
- `tests/support/mod.rs` re-exports the stubs and the DTOs that back the
  fixtures.

### 11.5 Functional tests

Per `.cursor/rules/testing.mdc`, new files:

- `tests/functional/observe_network_status.rs`
- `tests/functional/observe_gas_oracle.rs`
- `tests/functional/switch_chain.rs`
- `tests/functional/home_session.rs`

Each one includes at least one happy-path case and one failure-path case
using `rstest` + `pretty_assertions` and only talks to stubs.

### 11.6 UI adapter

- `src/adapters/ui/home.rs` exposes `HomeScreen` (pure render against a
  `HomeViewModel`). Rendering is done with `ratatui` but the widget does not
  own any async work; the application layer is responsible for driving
  `HomeSession`.
- A snapshot test under `tests/functional/home_screen_render.rs` uses
  `ratatui::backend::TestBackend` to assert the rendered frame contains the
  chain name, latest block number, the three gas tiers and (when applicable)
  the "disconnected" badge. A narrow-terminal case (width 60 × height 30)
  exercises the vertical-stack fallback from §2 and asserts the same
  content still renders when the two cards are stacked.

### 11.7 Wiring BDD steps

`tests/e2e/steps/home.rs` is rewritten to operate on a `HomeSession`
instance held by the `AppWorld`. Each step either prepares the stub state
(`given`), drives an application method (`when`) or asserts against
`HomeSession::view` (`then`). No step touches ratatui or stdin.

### 11.8 Acceptance

- `cargo test --test e2e` prints "5 scenarios (5 passed)" for
  `home.feature` once the degraded-state "cached snapshots" scenario is in.
- `cargo test --test functional` (or `cargo test`) passes all functional
  tests added under `tests/functional/`.
- `cargo clippy --all-targets` remains at zero errors.

## 12. WebSocket `newHeads` subscription (plan/15-backlog.md §8.2)

Status: **incremental** — port + stub + BDD scenario land together;
the real Alchemy WS adapter is still deferred.

The Home screen's MVP refreshes on a 6-second timer
(`infra/home_feed.rs::DEFAULT_REFRESH_PERIOD`). The backlog item under
`plan/15-backlog.md` §8.2 promotes this to a proper `eth_subscribe("newHeads")`
subscription so the latest block / base fee / gas oracle update within one
block of it being sealed. The work lands in three cohesive layers, with the
live WebSocket client explicitly deferred until the scope fits in one commit.

### 12.1 Domain

- `NewHead { chain: Chain, number: BlockNumber }` — the only field both
  observers actually care about. Timestamps and parent hashes stay on the
  existing `NetworkStatus` / `GasSnapshot` view models, which are still the
  source of truth for card rendering.
- No new error variants; transport issues still surface as
  `DomainError::ProviderUnavailable`.

### 12.2 Port

New trait under `src/application/ports/new_heads_stream.rs`:

```rust
pub trait NewHeadsStreamPort: Send + Sync {
    async fn subscribe(
        &self,
        chain: Chain,
    ) -> Result<UnboundedReceiver<NewHead>, DomainError>;
}
```

The stream is "best-effort": the receiver side drops when the upstream
connection fails, which is the signal for the dispatcher to flip the
`ConnectionStatus` to `Disconnected { reconnect_scheduled: true }` and
fall back to polling via the existing `home_feed` refresher.

### 12.3 Use case glue

`HomeSession` gains a single new method:

```rust
pub async fn on_new_head_event(&mut self, head: NewHead) -> Result<(), DomainError>
```

which is equivalent to `refresh()` but records the last head number so the
card can surface "last head: 21 345 679 (2s ago)" when we extend the layout
later. MVP just refreshes.

### 12.4 Adapter (deferred)

The real adapter lives in `src/adapters/rpc/new_heads.rs` and targets the
Alchemy WebSocket endpoint
`wss://{subdomain}.g.alchemy.com/v2/{api_key}`. It sends
`{"method": "eth_subscribe", "params": ["newHeads"]}` and translates each
incoming `newHeads` notification into a `NewHead` domain value. Because
`wiremock` cannot stand in for a WebSocket server, the adapter will rely on
an `async_trait`-free port backed by `tokio-tungstenite`; its own tests will
either be skipped until a WS test harness lands, or use an in-process
`tokio::net::TcpListener` echoing a canned JSON-RPC frame.

Until that adapter ships, `infra/mod.rs` wires a noop
`NewHeadsStream::Disabled` so polling remains the live behaviour. A
`TODO(plan/1-home.md §12.4)` comment points at this section.

### 12.5 Dispatcher wiring

`infra/home_feed.rs` grows a `start_with_stream(session, stream, period)`
variant that `select!`s between the WS stream and the timer tick:

- WS delivers a `NewHead` → call `session.on_new_head_event(head)` →
  publish the view.
- WS stream drops → mark the session disconnected (`on_connection_drop`),
  publish, and keep the timer running so the screen still refreshes.
- Timer fires → call `session.refresh()` as today (belt-and-suspenders:
  even while the WS is live, a low-frequency poll detects stale state
  when the provider silently pauses the stream).

### 12.6 BDD coverage

- `home.feature` already has the "Connection drop shows degraded state"
  scenario; §7 adds "Connection drop keeps last-known snapshots visible".
- A new scenario "New head event updates the Home view" drives a
  `StubNewHeadsStreamPort::push_head(...)` event through the session and
  asserts the view-model's `latest_block` changes without calling
  `on_new_head()` directly.

### 12.7 Tests and stubs

- `tests/support/stubs.rs` gains `StubNewHeadsStreamPort` with
  `push_head(chain, BlockNumber)` / `set_broken(bool)`. The stub stores
  outbound `UnboundedSender<NewHead>`s and broadcasts to all current
  subscribers.
- `tests/functional/observe_new_heads.rs` covers:
  - `it_emits_on_push_head` (happy path).
  - `it_surfaces_provider_unavailable_when_broken`.
- `tests/functional/home_session.rs` gains
  `it_refreshes_on_new_head_event` that pushes a `NewHead`, calls
  `on_new_head_event`, and asserts the view model was refreshed from the
  (re-primed) network stub.

### 12.8 Acceptance

- Port, stub and one happy-path scenario land in the same branch.
- The real Alchemy WS adapter lands in a follow-up branch referenced from
  `plan/15-backlog.md` §8.2 and §13 of this plan.
- Polling via `home_feed::start` remains the default entry point until the
  adapter ships; the refreshing behaviour from §11 stays unchanged for
  users.

## 13. Follow-up

- `src/adapters/rpc/new_heads.rs` — real Alchemy WebSocket adapter. Uses
  `tokio-tungstenite`; test harness TBD (see §12.4).
- "Last head age" badge on the Network card (requires an injected `Clock`
  port, tracked under `plan/15-backlog.md` §8.12).
- Remove the 6s polling fallback once the WS adapter has proven stable
  across the supported chains.
