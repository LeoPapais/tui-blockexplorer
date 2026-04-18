# 3 — Block Detail

Status: **done (expanded)** — the MVP slice from §11 is green plus
the five follow-ups previously tracked under §8.4 of
`plan/15-backlog.md`. Every shipped item is described in §12.
Whatever remains deferred (e.g. Beacon blob sidecars over the real
Beacon host) is called out in §13.

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

## 12. Follow-ups shipped (post-MVP)

The items below were queued as §8.4 of `plan/15-backlog.md`
("deferred from plan/3 §11.3") and are now part of the shipped
surface. Each sub-section lists the port trait, adapter target
endpoint, use case file path, BDD scenarios, functional tests and
fixtures that land together.

### 12.1 Clipboard bindings (`y` / `Y`)

`BlockDetailScreen` gains `y` / `Y` handlers that mirror the
`plan/3 §3` keybinding table. Copying does not yet reach the OS
clipboard (that belongs to the cross-cutting `ClipboardPort` tracked
in `plan/15-backlog.md §8.16`); instead the screen stores the
string in `last_copied_value: Option<String>` — identical contract
to `TxDetailScreen`. Tests inspect the field directly.

Semantics:

- Overview tab, `y` → copies the block hash.
- Transactions tab, `y` → copies the currently selected tx hash.
- Any tab, `Y` → copies the decimal block number.

Use cases touched: none — the binding is a pure UI concern.

Functional tests
(`tests/functional/block_detail_clipboard.rs`):

- `y_on_overview_copies_the_block_hash`.
- `y_on_transactions_copies_the_selected_tx_hash`.
- `uppercase_y_copies_the_block_number`.

### 12.2 Transaction categorisation (`TxCategory`)

Pure domain enum introduced in `src/domain/block.rs`:

```rust
pub enum TxCategory {
    Transfer,    // to == Some(_), input.is_empty() && value > 0
    Deploy,      // to == None (contract creation)
    Interaction, // otherwise (calls data, system calls, ...)
}
```

Exposed through a pure classifier
`TxCategory::classify(to: Option<Address>, input: &[u8], value: Wei)`
with exhaustive unit tests in `src/domain/block.rs`. `TxCategory`
also provides `badge()` (`"T"` / `"I"` / `"D"`) and `label()` so
future UI renderers do not re-derive the strings.

The category travels through the pipeline as a field of
`BlockTxReceipt` (see §12.3). Rendering the badge in the
Transactions tab is intentionally gated on the BlockFeed carrying
`BlockTxPage` updates — see §13 "UI wiring for receipts and
labels".

### 12.3 `BlockReceiptsPort` and `load_block_transactions`

Port under `src/application/ports/block_receipts.rs`:

```rust
pub trait BlockReceiptsPort: Send + Sync {
    async fn get_receipts(
        &self,
        id: BlockId,
        chain: Chain,
    ) -> Result<Vec<BlockTxReceipt>, DomainError>;
}
```

Adapter: `src/adapters/rpc/block_receipts.rs` —
`AlchemyBlockReceipts` wraps `eth_getBlockReceipts`. Endpoint:
JSON-RPC `eth_getBlockReceipts` against the Alchemy Node API.

Domain shape: `BlockTxReceipt { hash, tx_index, from, to, value,
gas_used, status, category }` (the `category` is filled in the use
case by running `TxCategory::classify`). `BlockTxPage { items,
next_cursor, total }` is the pagination envelope.

Use case:
`src/application/use_cases/load_block_transactions.rs::run(...)`:

- Inputs: `BlockReceiptsPort`, the already-loaded `Block`, a
  `Cursor { offset, page_size }`, and a
  `tokio_util::sync::CancellationToken`.
- Fetches the full receipts vector for the block once (Alchemy
  returns them all in one JSON-RPC call; no batching needed).
- Slices them into a page according to the cursor.
- Returns early when the cancellation token is triggered, so
  switching blocks cancels an in-flight page load.
- Never issues network calls when the offset is beyond the tail.

BDD: the pagination + badge rendering scenario is deferred until
the BlockFeed refactor described in §13 so the UI can observe
`BlockTxPage` updates. The domain / adapter / use-case layers are
covered end-to-end by the functional tests below.

Functional tests
(`tests/functional/load_block_transactions.rs`):

- `returns_the_first_page_when_offset_is_zero`.
- `slices_the_next_page_using_the_cursor`.
- `returns_empty_page_beyond_the_tail`.
- `categorises_rows_using_to_and_input`.
- `maps_failed_receipts_to_failed_status`.
- `returns_early_when_cancellation_token_is_triggered`.
- `propagates_domain_error_from_port`.

Fixtures:

- `rpc__eth_getBlockReceipts__21345678.json` (happy path, two
  transactions mirroring the block-reader fixture).
- `rpc__eth_getBlockReceipts__rate_limit.json` (HTTP 429 body used
  by adapter error tests).

### 12.4 `LabelPort` (validator / fee-recipient names)

Purpose: turn a raw address into a human label (e.g.
`Lido: Validator 7`, `Coinbase`, `0xPolygonSigner`) so the
Overview tab can render `Miner   0xab…cd  (Coinbase)` and the
Polygon signer row from `plan/15-backlog.md §3.5` grows a name
next to the recovered address.

Port: `src/application/ports/label.rs`:

```rust
pub trait LabelPort: Send + Sync {
    async fn label_for(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<Option<Label>, DomainError>;
}
```

Domain: `src/domain/label.rs`:

```rust
pub struct Label {
    pub name: String,
    pub source: LabelSource, // Etherscan { contract_name } | WellKnown
}
```

Adapters:

- `src/adapters/etherscan/label.rs::EtherscanLabel` — wraps
  `contract/getsourcecode` and maps `ContractName` onto
  `Label { name, source: Etherscan }`. Endpoint:
  `https://api.etherscan.io/v2/api?module=contract&action=getsourcecode`.
- `src/adapters/labels/well_known.rs::WellKnownLabels` — tiny
  checked-in table keyed by `(chain, address)`. Bootstraps Polygon
  mainnet with a handful of validator signers so §3.5 renders a
  readable row out of the box.
- `src/adapters/labels/composite.rs::CompositeLabels` — tries
  well-known first, falls back to the Etherscan adapter. Mirrors
  `CompositeSignatureDirectory`.

Use case: `load_block_overview` is reshaped to return a
`BlockOverview { block, miner_label, signer_label }` view-model.
Missing labels stay `None`; label-port errors are non-fatal.

BDD: the labelled-signer scenario lands with the BlockFeed
refactor tracked in §13. The existing
`Scenario: Polygon block shows extraData signer` stays green (raw
signer row) in the meantime; the domain / adapter / use-case
layers of the label lookup are covered by the functional tests
below.

Functional tests
(`tests/functional/load_block_overview.rs` — extend) +
`tests/functional/label_composite.rs`:

- `uses_the_well_known_table_first`.
- `falls_back_to_etherscan_when_well_known_is_empty`.
- `returns_none_when_both_sources_return_none`.
- `label_port_error_does_not_break_overview_load`.

Fixtures:

- `etherscan__getsourcecode__label_coinbase_hot_wallet.json`.
- `etherscan__getsourcecode__label_unverified.json` (used to
  assert graceful degradation).

### 12.5 `load_block_withdrawals` (post-Shanghai)

Domain: `Block` grows a `withdrawals: Vec<Withdrawal>` field where
`Withdrawal { index, validator_index, address, amount_gwei }`. The
Alchemy block reader parses the `withdrawals` RPC array onto that
vector.

Use case:
`src/application/use_cases/load_block_withdrawals.rs::run(block)` —
pure passthrough returning the slice so the screen can paginate and
render without re-calling the reader.

Beacon `blob_sidecars` stays deferred (see §13). The "Blobs /
Withdrawals" tab ships with:

- a top block showing the withdrawals.
- a bottom block showing the count of blob-carrying transactions
  inferred from `TxType::Blob` in the receipts page, plus a
  disclaimer "blob sidecar sizes require the Beacon API —
  deferred, see plan/3 §13".

BDD: `tests/e2e/features/block_detail.feature` —
`Scenario: Blobs / Withdrawals tab renders withdrawals` opens the
tab and asserts on a validator-index row.

Functional tests
(`tests/functional/load_block_withdrawals.rs`):

- `returns_the_parsed_withdrawals_verbatim`.
- `empty_block_returns_an_empty_vec`.

Fixtures:

- `rpc__eth_getBlockByNumber__with_withdrawals.json` — Shanghai-era
  block carrying withdrawals.

## 13. Still deferred

- **UI wiring for receipts and labels.** Domain types, ports,
  adapters and use cases for `BlockReceiptsPort` +
  `load_block_transactions` (§12.3) and `LabelPort` +
  `run_with_labels` (§12.4) have all shipped and are exercised
  end-to-end by the functional tests. Rendering the category
  badge in the Transactions tab and the miner / signer label in
  the Overview tab requires refactoring `BlockFeed` so the
  resolver task emits a richer `BlockOverviewUpdate { block,
  miner_label, signer_label, tx_page }` envelope. Promoting this
  work mechanically unblocks the
  `Scenario: Paginate transactions tab` and
  `Scenario: Polygon block shows labelled signer` BDD slots that
  §12.3 and §12.4 reserved.
- Beacon blob sidecars (`/v1/beacon/blob_sidecars/{id}`). Separate
  host, different credential (beacon node / Alchemy Beacon API),
  non-trivial retry semantics. The Blobs half of the
  Blobs / Withdrawals tab renders a clearly-marked placeholder
  until a `BeaconApiPort` lands.
- `eth_blobBaseFee` on the block header. Same tab as sidecars; no
  adapter wired yet.
- Reconstructing the Bor seal hash inside `AlchemyBlockReader` so
  the Polygon signer row recovers live (see
  `plan/15-backlog.md §3.5`). The pure recoverer + the label row
  are in place; only the RLP reconstruction is outstanding.
- Real OS clipboard wiring for `y` / `Y`. Today the screen stores
  the last yanked value so functional tests can inspect it; the
  cross-cutting `ClipboardPort` (see `plan/15-backlog.md §8.16`)
  will promote that field into an `arboard` call.
