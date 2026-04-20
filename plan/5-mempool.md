# 5 — Mempool

**Decision (April 2026): abandoned.** The mempool stream screen is not part of
the product anymore: all production code, ports, adapters, tests, and Cucumber
features for this slice have been **removed**. The text below is kept only as an
archival specification if the team ever revisits a live pending-tx view.

---

_Status before removal was:_ **done** (MVP + §11.3 follow-ups shipped, except the
reconnect-capable Alchemy adapter which stayed incremental — see §11.3.4 and
§11.3.5).

Live stream of pending transactions as seen by Alchemy's mempool. Important caveat:
this is the mempool Alchemy observes, not a globally complete mempool; a globally
complete view does not exist by design on public chains.

## 1. Purpose and user goals

- See transactions right as they appear in the mempool.
- Filter the stream by sender, receiver, or minimum value.
- Drill into a pending tx to see the same TxDetail screen (with pending-aware fields).

## 2. Layout

```
+-- Breadcrumb ----------------------------------------------------+
| Home > Mempool                                                   |
+-- Filters -------------------------------------------------------+
| from: <any>   to: <any>   min value: <any>      [f] edit         |
+-- Stream --------------------------------------------------------+
| wait   hash               from             to              value |
| 0.3s   0x8f3c...abcd      0xd8dA...        0xa0b8...USDC   0 ETH |
| 0.4s   0x9a2e...0001      0x1234...        0x1111...       1 ETH |
| ...                                                              |
+-- Status bar ----------------------------------------------------+
| p pause  f filters  Enter open  y copy hash                      |
+------------------------------------------------------------------+
```

Each row updates as the tx propagates: if it gets mined, it disappears (moves to the
latest block); if it is replaced, it is removed and replaced with the new one.

## 3. Keybindings

| Key     | Action                                   |
|---------|------------------------------------------|
| `p`     | Pause / resume the stream                |
| `f`     | Edit filters (opens modal)               |
| `Enter` | Open selected pending tx in TxDetail     |
| `y`     | Copy hash                                |
| `c`     | Clear current list                       |

## 4. Use cases

### 4.1 `ObservePendingTxs`

- **Input**: current chain, filter spec.
- **Output**: stream of `PendingTx` deltas (`Added`, `Replaced`, `Removed`).
- **Ports**: `PendingTxStreamPort`.
- **Behaviour**:
  1. Open `alchemy_pendingTransactions` subscription with filter translated from the
     domain filter spec.
  2. Buffer incoming txs, expose as a sliding window of the latest N (default 200).
  3. Remove items whose hash appears in a freshly mined block (from `newHeads` hook).

### 4.2 `FilterPendingTxs`

- **Input**: `FilterSpec`.
- **Output**: updated filter applied to both server-side subscription (when possible)
  and client-side post-filter.
- **Ports**: `PendingTxStreamPort::update_filter`.

## 5. Ports required

- `PendingTxStreamPort`:
  - `async fn subscribe(chain, filter) -> Stream<PendingTxDelta>`.
  - `async fn update_filter(filter)`.
- `NetworkStatusPort::subscribe_heads(chain) -> Stream<NewHead>` (reused for mined
  detection).

## 6. Data sources

- Alchemy subscription `alchemy_pendingTransactions` (preferred; supports filters).
- Alchemy subscription `newPendingTransactions` (fallback; hashes only, hydrate with
  `eth_getTransactionByHash`).
- Alchemy subscription `newHeads` (for mined detection).

## 7. BDD scenarios (`tests/e2e/features/mempool.feature`)

```gherkin
Feature: Mempool stream

  Background:
    Given the user is on Mempool
    And the active chain is "ethereum"

  Scenario: Stream starts on entry
    When the stub emits three pending txs
    Then three rows appear in the list within 500ms

  Scenario: Filter by sender
    Given three pending txs from different senders are emitted
    When the user opens filters and sets "from" to "0xd8dA..."
    Then only txs from "0xd8dA..." remain in the list

  Scenario: Pause and resume
    Given the stream is active and one tx is visible
    When the user presses "p"
    And the stub emits two more pending txs
    Then the list does not change
    When the user presses "p" again
    Then the two new txs appear

  Scenario: Pending tx is mined and disappears
    Given a pending tx with hash "0x8f3c..." is visible
    When the stub emits a newHead that includes that hash
    Then the row is removed from the list

  Scenario: WebSocket disconnect is handled
    Given the stream is active
    When the WS connection drops
    Then a "reconnecting" badge appears in the status bar
    And after reconnect the stream resumes
```

## 8. Functional tests

- `ObservePendingTxs`: delta ordering (adds preserve arrival order); sliding-window
  eviction keeps newest; mined detection removes exactly the right hashes.
- `FilterPendingTxs`: filter update reflects in outgoing subscription params;
  client-side fallback when subscription does not support a filter.

## 9. Fixtures

- `rpc__alchemy_pendingTransactions__sequence_3.json`
- `rpc__newPendingTransactions__hashes_only.json`
- `rpc__newHeads__with_mined_hashes.json`
- `rpc__eth_getTransactionByHash__hydrate_pending.json`

## 10. Resolved questions

- **Default filter on launch**: `PendingTxFilter::default()` (every
  predicate is `None`). Showing "all pending txs the provider emits"
  is the most useful MVP default: it gives the user a signal of
  activity immediately, and filters become an additive opt-in via
  the `f` key (filter modal, still deferred §11.3.3). The Mempool
  screen therefore boots with no filter applied both in demo mode
  and on live `cargo run`. Codified by
  `tests/functional/mempool_screen.rs::default_filter_is_empty` and
  the BDD scenario "Mempool opens with no active filters" in
  `mempool.feature`.
- **Retention of mined txs**: zero; they disappear immediately. The
  user can still open TxDetail for a recently seen hash via Search.

## 11. Implementation plan

Two slices. The WebSocket adapter that actually talks to Alchemy is
explicitly deferred (section 11.3) because it requires a brand new
adapter tier; everything else ships behind a trait so the WS
implementation will plug in without refactors.

### 11.1 Slice A — domain + port + use case + stubs

Domain additions (`src/domain/`):

- `mempool.rs`
  - `PendingTx { hash, from, to, value }` — lean on purpose; the
    Mempool screen only needs enough to render a row and drill into
    TxDetail.
  - `PendingTxEvent { Added(PendingTx), Removed(TxHash) }` — maps
    the Alchemy delta shape (Alchemy emits whole tx objects on add
    and we synthesise removals from `newHeads` hashes in future
    slices).
  - `PendingTxFilter { from: Option<Address> }` — single-axis for
    MVP; additional filters land with the filter-modal slice.

Port (`src/application/ports/pending_tx_stream.rs`):

```rust
pub trait PendingTxStreamPort: Send + Sync {
    async fn subscribe(
        &self,
        chain: Chain,
        filter: PendingTxFilter,
    ) -> Result<UnboundedReceiver<PendingTxEvent>, DomainError>;
}
```

Use case `observe_pending_txs` is a passthrough that returns the
subscribed receiver.

Stub `StubPendingTxStreamPort` in `tests/support/stubs.rs` creates
the channel eagerly and exposes `push_added(PendingTx)` and
`push_removed(TxHash)` helpers so tests can drive it deterministically.

Functional tests `tests/functional/observe_pending_txs.rs`:
- events arrive in insertion order;
- dropping the port does not close the receiver mid-stream;
- multiple subscribers each receive their own channel.

### 11.2 Slice B — UI + BDD + wiring

- `src/adapters/ui/mempool.rs` — `MempoolScreen` with the rolling
  list, client-side filter (applied on `tick` drain), pause flag,
  clear-on-`c` and `Enter` opens a TxDetail via an injected factory.
- The Home screen gains an optional `mempool_factory` set by the
  composition root. Pressing `m` on Home pushes the factory's result.
- `infra::run` wires an `EmptyPendingTxStream` adapter for the live
  path: it subscribes to a dead channel so the Mempool opens and
  renders an "waiting..." empty-state message. The noop adapter is
  isolated in `src/infra/mempool_feed.rs` so the future WS adapter
  drops in without touching `run`.

BDD scenarios in `tests/e2e/features/mempool.feature`:

- Stream shows three pending txs when the stub emits three.
- Filter by sender drops rows that do not match the `from` address.
- Pause blocks new events; resume drains them.
- Removed events delete the matching row.

Scenarios from plan section 7 that depend on WebSocket reconnect
are intentionally skipped until the WS adapter lands.

### 11.3 Follow-up status (April 2026, branch `probe/8.6-mempool-followups`)

Numbered items 1–4 below mirror `plan/15-backlog.md` §8.6 one-to-one
so the consolidated backlog and this plan stay in lock-step.

#### 11.3.1 Default filter on launch — **shipped**

`PendingTxFilter::default()` is the launch state for every entry
point (`infra/mempool_feed.rs` live path, demo factory, and the
BDD harness). The decision, semantics and tests are documented in
§10 above. No port change required; this is a pure UX decision
pinned by a functional test (`default_filter_is_empty`) and a BDD
scenario ("Mempool opens with no active filters").

#### 11.3.2 `PendingTxStreamPort::update_filter` wiring — **shipped**

- **Port extension**. `PendingTxStreamPort` grows a new method:

  ```rust
  fn update_filter(
      &self,
      chain: Chain,
      filter: PendingTxFilter,
  ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send;
  ```

  The method is best-effort: providers that do not support
  server-side filtering translate the call into a re-subscribe with
  a fresh predicate; the stub and the `EmptyPendingTxStream`
  implement a noop that records the filter for assertions.

- **Use case**. `application/use_cases/observe_pending_txs.rs`
  exposes `update_filter(port, chain, filter)` as a thin wrapper
  alongside the existing `run` entry point; keeps the screen free
  of direct port dependencies.

- **UI plumbing**. The `MempoolScreen` gains an optional
  `filter_control: UnboundedSender<PendingTxFilter>` handed to the
  composition root. `set_filter` broadcasts the new predicate onto
  that channel (fire-and-forget — if the control task is gone, the
  client-side filter still prunes); `infra/mempool_feed.rs` spawns
  a tiny drain task that reads the channel and calls
  `port.update_filter(...)` via the use case.

- **Semantics**. When `set_filter` fires the screen prunes the
  visible list immediately so the user sees the effect — we decided
  against the alternative ("new filter only affects future events")
  because the existing BDD scenario "Filter by sender drops
  non-matching rows" and user expectation both point at prune. The
  server-side update is an optimisation to reduce bandwidth on
  future notifications. This is pinned by
  `tests/functional/mempool_screen.rs::set_filter_forwards_to_control_channel`
  and by the existing pruning scenario.

- **Stub + tests**. `StubPendingTxStreamPort` records every
  `update_filter` call in insertion order so functional tests can
  assert order and content
  (`tests/functional/observe_pending_txs.rs::update_filter_is_recorded_in_order`).

#### 11.3.3 Reconnecting badge — **shipped**

The Home shell already models connection state with
`application::ConnectionStatus { Connected, Disconnected {
reconnect_scheduled } }`. The Mempool screen reuses the same enum
verbatim:

- **Screen state**. `MempoolScreen` gains a
  `stream_state: ConnectionStatus` field (default `Connected`) and
  accepts an optional `status_rx: UnboundedReceiver<ConnectionStatus>`
  built by `infra/mempool_feed.rs::mempool_status_feed()`.

- **Drain contract**. `tick()` drains both feeds:
  - every `PendingTxEvent` as today;
  - every `ConnectionStatus` update, mapping
    `Disconnected { reconnect_scheduled: true }` to a
    `[reconnecting]` badge rendered in the header row. When the
    events channel itself sees `TryRecvError::Disconnected` (the
    upstream sender was dropped), the screen auto-flips to
    `Disconnected { reconnect_scheduled: true }` — this guarantees
    the badge still appears when a stub or live adapter dies
    without publishing a status frame.

- **Event retention**. Existing items stay in place when the stream
  drops (up to the `WINDOW_SIZE` sliding-window cap of 200 already
  enforced on `Added`; the plan calls for 100 but the MVP uses 200
  so we keep the richer buffer — tracked as a minor discrepancy in
  `plan/15-backlog.md` §8.6). No forced prune on disconnect.

- **BDD**. `Stream drop shows reconnecting badge and preserves
  buffered events` drives an `emit_stream_drop` helper on
  `StubPendingTxStreamPort` (and the matching
  `emit_status(ConnectionStatus)`) through the world and asserts on
  both the badge and the list length.

- **Snapshot**. `tests/functional/mempool_screen_render.rs` renders
  the screen in each of the three states and asserts the badge
  string via `TestBackend`.

#### 11.3.4 Alchemy WebSocket adapter — **partial, incremental**

- **What shipped**: the port now has a concrete
  `AlchemyPendingTxStream` under
  `src/adapters/rpc/pending_tx_stream.rs`. The adapter:
  - Connects to `wss://{subdomain}.g.alchemy.com/v2/{api_key}`
    (URL built from `Chain::alchemy_ws_url_template` — already
    present on `Chain`).
  - Sends `eth_subscribe` with
    `["alchemy_pendingTransactions", { "fromAddress": "0x…" }?]`
    honouring the `PendingTxFilter::from` predicate when set.
  - Translates each `eth_subscription` notification whose `result`
    is a full tx object into `PendingTxEvent::Added(PendingTx)`,
    using the lean domain shape (`hash, from, to, value`).
  - `update_filter(chain, filter)` forwards the new predicate to
    the in-flight task via an internal control mpsc; the task
    tears down the current `eth_subscribe` subscription and starts
    a fresh one.
  - Handles connection loss by closing the outbound receiver; the
    UI flips to `Disconnected { reconnect_scheduled: true }` via
    the status feed from §11.3.3 and the outer retry loop (future
    work) is expected to re-subscribe.

- **What is still deferred (still §11.3.4 follow-ups)**:
  1. `newPendingTransactions` fallback + hydration via
     `eth_getTransactionByHash` when `alchemy_pendingTransactions`
     returns `method not found` on non-Alchemy endpoints (Alchemy
     is the only MVP provider, so the fallback is cosmetic today).
  2. `newHeads` subscription wiring for mined-removal synthesis:
     the adapter does not emit `PendingTxEvent::Removed` yet. The
     screen continues to rely on `StubPendingTxStreamPort::push_removed`
     in BDD scenarios; live users will see their mined pending txs
     stay on the list until the window rolls over.
  3. Reconnect loop: today the adapter task exits when the WS
     socket closes. A future slice adds exponential-backoff retry
     with jitter (cf. `.cursor/rules/external-apis.mdc`).

- **Testing**. `tests/support/ws_harness.rs` owns a tiny in-process
  WS server (`CannedWsServer`) built on `tokio::net::TcpListener` +
  `tokio_tungstenite::accept_async`. It speaks enough JSON-RPC to
  accept an `eth_subscribe("alchemy_pendingTransactions", …)`
  request and then streams back a sequence of canned
  `eth_subscription` frames. The functional test
  `tests/functional/alchemy_pending_tx_stream.rs` asserts that
  `subscribe(...)` yields `PendingTxEvent::Added` for the canned
  frames in order. `reqwest::Client::new()` is still banned from
  tests; the harness only uses `tokio` and `tokio_tungstenite` so
  the `.cursor/hooks` guard stays green.

- **Dependency additions**: `tokio-tungstenite` (workspace dep,
  rustls features to match `reqwest`). No other crate change.

#### 11.3.5 Still deferred (post-8.6)

- Filter modal (`f` key) with multi-axis filters (to, min value).
- Pending-aware TxDetail view (status "pending" when opened from
  Mempool before a block mines the tx).
- The `newPendingTransactions` + hydration fallback and the
  `newHeads` mined-removal synthesis called out in §11.3.4 above.
- Reconnect loop and exponential backoff for
  `AlchemyPendingTxStream` (tracked once the Alchemy adapter for
  Home §12.4 grows the same capability).
