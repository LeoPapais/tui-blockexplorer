# 3 — Block Detail

Status: in progress. Slices A (domain + use case + stubs) and B
(Alchemy adapter) landed first; Slice C (UI + BDD) finishes the
plan. Full status tracked in section 11.

Everything about one specific block. Reached from Search or from any screen that
links to a block (Tx overview, Address transfers). Tabs are used to keep the view
compact: Overview, Transactions, Blobs / Withdrawals.

## 1. Purpose and user goals

- Give the full technical picture of a block without leaving the terminal.
- Let the user drill into any of the block's transactions in one keystroke.
- Navigate chronologically with `[` and `]`.

## 2. Layout

```
+-- Breadcrumb ----------------------------------------------------+
| Home > Block #21,345,678                                         |
+-- Tabs ----------------------------------------------------------+
| [Overview]  Transactions  Blobs / Withdrawals                    |
+-- Content -------------------------------------------------------+
| Hash       0x...                                                 |
| Parent     0x...                                                 |
| Timestamp  2024-06-01 10:12:43 UTC  (3m 12s ago)                 |
| Validator  0xAbCd...  (Lido: Validator 7)                        |
| Gas used   12,432,110 / 30,000,000   (41.4%)                     |
| Base fee   11.4 gwei      Burned   0.142 ETH                     |
| Size       102 kB         Txs     142                            |
| Extra data 0x...                                                 |
| Withdrawals count 16       Blob txs 3                            |
+-- Status bar ----------------------------------------------------+
| [ prev   ] next   Enter open tx   y copy hash                    |
+------------------------------------------------------------------+
```

Tabs:

- **Overview**: header fields as above.
- **Transactions**: compact table of the txs in the block (`index`, `hash`, `from`,
  `to`, `value`, `gas used`, `status`).
- **Blobs / Withdrawals**: two sub-panels. Withdrawals list (validator index, amount,
  address). Blob-carrying txs (tx hash, blob count, blob gas used, blob gas price).

## 3. Keybindings

| Key       | Action                                       |
|-----------|----------------------------------------------|
| `Tab`     | Next tab                                     |
| `Shift+T` | Previous tab                                 |
| `[`       | Previous block                               |
| `]`       | Next block                                   |
| `Enter`   | Open selected transaction (Transactions tab) |
| `y`       | Copy block hash                              |
| `Y`       | Copy block number                            |

## 4. Use cases

### 4.1 `LoadBlockOverview`

- **Input**: block identifier (number or hash), chain.
- **Output**: `BlockOverview` (fields as laid out in section 2).
- **Ports**: `BlockReaderPort::get(id, chain)`.
- **Behaviour**: fetches the block once, maps to domain entity. For the validator
  label, asks `LabelPort` but failure is non-fatal (returns raw address).

### 4.2 `LoadBlockTransactions`

- **Input**: block identifier, chain, pagination cursor.
- **Output**: a page of `TxSummary` rows.
- **Ports**: `BlockReaderPort::get_with_full_txs(id, chain)`,
  `BlockReceiptsPort::get_receipts(id, chain)`.
- **Behaviour**: fetches txs and receipts in parallel, merges for status and gas used.

### 4.3 `LoadBlockWithdrawals`

- **Input**: block identifier, chain.
- **Output**: `Vec<Withdrawal>` plus `Vec<BlobCarryingTx>`.
- **Ports**: `BlockReaderPort::get(id, chain)`, `BeaconPort::get_blob_sidecars(id)`
  (optional; missing adapter is acceptable for non-Ethereum chains).

## 5. Ports required

- `BlockReaderPort`: `get_by_number`, `get_by_hash`, `get_with_full_txs`.
- `BlockReceiptsPort`: `get_receipts(id, chain)` (single batched call using
  `eth_getBlockReceipts`).
- `BeaconPort` (optional): `get_blob_sidecars(block_id)`.
- `LabelPort`: `label_for(address, chain)`.

## 6. Data sources

All via Alchemy unless noted:

- `eth_getBlockByNumber` / `eth_getBlockByHash` (with `fullTxs=true` for the
  Transactions tab).
- `eth_getBlockReceipts`.
- `eth_blobBaseFee` for the blob panel base rate.
- Beacon API `/v1/beacon/blob_sidecars/{block_id}` for blob sidecar metadata (only
  Ethereum mainnet).
- Etherscan V2 label lookup for the validator / fee recipient.

## 7. BDD scenarios (`tests/e2e/features/block_detail.feature`)

```gherkin
Feature: Block detail

  Background:
    Given the user is on Home
    And the active chain is "ethereum"

  Scenario: Load block by number
    When the user opens Search with "21345678"
    And selects the block candidate
    Then the BlockDetail screen shows the Overview tab
    And the hash, parent hash and gas used match the stub

  Scenario: Paginate transactions tab
    Given the user is on BlockDetail for block 21345678
    When the user switches to the Transactions tab
    Then the first page of transactions is shown
    When the user scrolls past the end of the first page
    Then the next page is requested once and merged

  Scenario: Open a transaction from the block
    Given the user is on the Transactions tab of BlockDetail 21345678
    When the user selects the second transaction and presses Enter
    Then the TxDetail screen is pushed
    And pressing Esc returns to BlockDetail on the Transactions tab

  Scenario: Handle missing block
    When the user opens BlockDetail for block 99999999999
    Then an error modal appears with text "block not found"
    And pressing Esc returns to the previous screen

  Scenario: Previous and next navigation
    Given the user is on BlockDetail for block 21345678
    When the user presses "["
    Then BlockDetail loads block 21345677 without pushing a new screen
    When the user presses "]"
    Then BlockDetail loads block 21345678 again
```

## 8. Functional tests

- `LoadBlockOverview`: happy path by number; by hash; missing block returns
  `DomainError::NotFound`; label lookup failure does not fail the overall call.
- `LoadBlockTransactions`: receipts merged correctly; pagination cursor correctness;
  cancellation on rapid tab changes.
- `LoadBlockWithdrawals`: no blob sidecar adapter configured degrades gracefully.

## 9. Fixtures

- `rpc__eth_getBlockByNumber__21345678_full.json`
- `rpc__eth_getBlockByNumber__not_found.json`
- `rpc__eth_getBlockReceipts__21345678.json`
- `beacon__blob_sidecars__21345678.json`
- `etherscan__label__validator_0xabcd.json`

## 10. Open questions

- Do we precompute transaction categorisation (transfer vs contract interaction vs
  deployment) inside `LoadBlockTransactions`? Yes, based on `to` and input bytes.

## 11. Implementation plan

Delivered in three slices, each its own commit with the red->green loop.

### 11.1 Slice A — domain + port + use case + stubs

Scope kept tight: Overview tab only plus a flat transaction list (tx
hashes). Blobs / Withdrawals / Beacon / Label integration are all
deferred to a later phase since they require extra adapters we do
not own yet.

Domain additions (`src/domain/`):

- `timestamp.rs` — `UnixTimestamp(u64)` newtype with `seconds` /
  `from_seconds` helpers. Used by the Block entity.
- `block.rs` — `BlockId { Number(BlockNumber), Hash(BlockHash) }` plus a
  rich `Block` entity:
  - `chain`, `number`, `hash`, `parent_hash`.
  - `timestamp` (unix), `miner` (Address).
  - `gas_used`, `gas_limit`, `base_fee` (optional pre-1559 / some L2s).
  - `size` (bytes), `extra_data` (raw bytes).
  - `tx_hashes: Vec<TxHash>` — flat list; receipt / from-to / status
    enrichment waits for the receipts adapter.

Port (`src/application/ports/block_reader.rs`):

```rust
pub trait BlockReaderPort: Send + Sync {
    async fn get(&self, id: BlockId, chain: Chain)
        -> Result<Option<Block>, DomainError>;
}
```

Use case (`src/application/use_cases/load_block_overview.rs`): simple
passthrough that translates the port's `Ok(None)` into
`DomainError::NotFound` so the UI can show a "block not found"
message without null-checking further.

Stubs (`tests/support/stubs.rs::StubBlockReaderPort`): in-memory map
keyed by both number and hash for each inserted `Block`.

Functional tests (`tests/functional/load_block_overview.rs`):

- happy path by number.
- happy path by hash.
- missing block returns `DomainError::NotFound`.
- multiple blocks inserted: lookup by either key works independently.

### 11.2 Slice B — Alchemy adapter

New adapter `src/adapters/rpc/block_reader.rs` implementing
`BlockReaderPort` via `eth_getBlockByNumber` / `eth_getBlockByHash`
with `fullTransactions=false`. Response parsing pulls every field
listed under 11.1, mapping hex numbers and byte arrays onto the
domain types.

Tests `tests/functional/alchemy_block_reader.rs` use wiremock:
happy path by number, happy path by hash, null result maps to
`Ok(None)`. Fixture
`tests/fixtures/rpc__eth_getBlockByNumber__21345678_full.json`.

### 11.3 Slice C — UI and navigation

- `src/adapters/ui/block_detail.rs` hosting `BlockDetailScreen`.
  - Tabs: Overview (rendered from the `Block` entity), Transactions
    (list of `tx_hashes` with index + short hash).
  - Keybindings: `Tab`/`Shift+T` switch tabs; `[` / `]` trigger
    prev/next block loads through a feed channel; `Enter` on a
    transaction pushes a `DetailPlaceholderScreen` until plan/4
    replaces it; `y` / `Y` copy hash / number via arboard.
- `src/infra/block_feed.rs` — spawn helper that owns a
  `BlockReaderPort`, receives `BlockId` requests on an input channel,
  and sends fresh `Block` values back on an output channel. Mirrors
  `search_feed.rs`.
- Search integration: the `DetailFactory` used by the `SearchScreen`
  now produces a full `BlockDetailScreen` when the resolved entity is
  a `Block`, falling back to the existing `DetailPlaceholderScreen`
  for the other kinds.
- BDD adjustments in `tests/e2e/features/block_detail.feature`:
  - `Load block by number`.
  - `Open a transaction from the block` (pushes the placeholder in
    the MVP slice; will naturally start pushing TxDetail when plan/4
    lands).
  - `Handle missing block` (resolve fails -> modal / error banner on
    the Search screen, which is where the flow starts).
  - `Previous and next navigation`.
- Deferred: the Transactions pagination scenario (requires receipts
  + paging).

Acceptance: every remaining BDD scenario green, plan flipped to
`done` in `plan/README.md`, clippy clean.
