# 5 — Mempool

Status: **done** (MVP scope) — domain, port, use case and
`MempoolScreen` are all in place, the four BDD scenarios in
`tests/e2e/features/mempool.feature` are green. The Alchemy WebSocket
adapter that turns this screen into a live view stays deferred
(section 11.3); `cargo run` currently opens the screen subscribed to
an `EmptyPendingTxStream` and shows the "waiting..." state until
that adapter lands.

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

## 10. Open questions

- Default filter on launch: none. User decides.
- Retention of mined txs: zero; they disappear immediately. The user can still open
  TxDetail for a recently seen hash via Search.

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

### 11.3 Deferred (next slices under this plan)

1. **Alchemy WebSocket adapter**: drives
   `alchemy_pendingTransactions` and translates the notifications
   into `PendingTxEvent::Added`. Needs the `newHeads` subscription
   too for mined-removal synthesis.
2. **Reconnect and "reconnecting" banner** in the UI, plus the
   associated BDD scenario.
3. **Filter modal** (`f` key) with multi-axis filters (to, min
   value).
4. **Pending-aware TxDetail view** (status rendered as "pending"
   when opened from Mempool before a block mines the tx).
