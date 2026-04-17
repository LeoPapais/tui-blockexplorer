# 5 — Mempool

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
