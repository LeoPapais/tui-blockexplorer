# 18 — Shell chrome, live feeds, address/tx polish, contract navigation

Status: **ready** (accepted product goals; implement in ordered slices).

**Progress (repo):** Slices **A–E** are implemented. **D** and **E** landed via
isolated **`/worktree`** + subagent + cherry-pick/merge, then **`/delete-worktree`**
and gates on **main** (see [`.cursor/README.md`](../.cursor/README.md)).
Remaining slices **F–H**: same pipeline with **`plan18-worktree-slice`**.

Cross-references: [plan/0-general-architecture.md](0-general-architecture.md) (shell),
[plan/1-home.md](1-home.md) §12.5 (WebSocket home feed),
[plan/4-tx-detail.md](4-tx-detail.md),
[plan/6-address-detail.md](6-address-detail.md),
[plan/16-unified-address-detail.md](16-unified-address-detail.md),
[plan/17-navigable-values.md](17-navigable-values.md).

## 1. Purpose

Deliver UX and data correctness improvements requested for Home, Address
(EOA and ERC-20-as-address), Transaction detail, and global shell behaviour:
breadcrumb trail, consistent unfocused borders, WebSocket-driven home
refresh, correct portfolio vs transfer lists, incremental loading where
feeds are slow, scrolling and clipboard fixes, and contract/token sub-tab
navigation parity.

## 2. Non-goals

- Replacing Alchemy with a custom indexer.
- Mouse support (remains out of scope per TUI rules).

## 3. Implementation slices (serial merge order)

Each slice should ship with the tests for that slice before production code
lands (TDD + BDD where user-visible). Merge order reduces rebase pain: shell
chrome and focus tokens first, then feeds, then new ports, then large UI
state machines.

### Slice A — Shell header, breadcrumb, border semantics

**Goals**

0. **Header row**: Reserve a top row (or two) in `draw_screen_with_footer` /
   equivalent so every non-modal screen shows a **breadcrumb** of the stack:
   `Home > … > current` using [`breadcrumb::render_breadcrumb`](src/adapters/ui/breadcrumb.rs).
   Enrich `Screen::title()` strings where needed so crumbs are human-meaningful
   (e.g. include shortened hash / block number where the generic title is too
   vague), without breaking the stack contract in `plan/0-general-architecture.md`.

1. **Unfocused borders**: Any pane that does **not** own keyboard focus must use
   the same dim grey as an **unfocused tab strip** (`palette.muted`), not
   `foreground` (which reads as bright white on the default dark palette).
   Apply to:
   - Primary **header** block (address hash, tx hash, block id) on Address,
     Transaction, and Block detail screens when focus is on main tabs, sub-tabs,
     or body.
   - **Body** block when focus is on a tab strip (fix `detail_body_border_style`
     so non-`Content` focus uses `muted`).

**Ports / domain**: None.

**Tests**: Unit tests for `detail_body_border_style` / new `chrome_border_style`
helpers; snapshot or functional test for breadcrumb string given a stub stack.

---

### Slice B — Home: refresh on every new head via WebSocket

**Goals**

- In live mode (`build_live_stack`), compose `AlchemyNewHeadsStream` with the
  existing HTTP `RpcClient` session: build `wss://{subdomain}.g.alchemy.com/v2/{key}`
  parallel to `alchemy_url`.
- Call `infra::home_feed::start_with_stream` instead of `start` when the stream
  is constructed; keep the polling timer as belt-and-suspenders per
  `plan/1-home.md` §12.5.

**Ports**: Existing `NewHeadsStreamPort` only.

**Tests**: Extend `tests/functional/home_feed.rs` (or infra-focused test) so the
streamed loop is covered with `StubNewHeadsStreamPort`; no real network.

---

### Slice C — Address EOA: Portfolio tab, balances, naming

**Goals**

- Rename main tab **Tokens** → **Portfolio** (labels, titles, help text,
  plan/16 table row).
- **Native balance**: Show chain native asset balance on the Portfolio tab
  (from `eth_getBalance` or existing address overview if already present;
  otherwise extend overview or portfolio use case — document choice).
- **ERC-20 list correctness**: Today `AlchemyPortfolio` truncates to
  `MAX_HOLDINGS` (20) after sorting by raw balance — high-value tokens can still
  be missing if metadata batch fails or ordering is wrong. Raise cap and/or add
  **paged** loading (cursor or offset) until the product owner cap is defined;
  add regression fixture for a wallet with many non-zero tokens.

**Ports**: Possibly extend `PortfolioPort` or add `NativeBalancePort` if a clean
boundary is needed; prefer reusing `AddressReaderPort` if balance already exists
on `AddressOverview`.

**Tests**: Functional tests for `load_address_portfolio` / adapter with fixtures
including native row and >20 holdings page.

---

### Slice D — Address EOA: split **Transactions** vs **ERC-20 Transfers**

**Goals**

- Current **Transactions** tab is backed by `TransfersPort::get_for_address`
  (`alchemy_getAssetTransfers`) — asset transfers, not executed transactions.
- Add a main tab (or rename pair): **Transactions** = externally indexed normal
  txs where `from` or `to` equals the address; **Transfers** (or **Token
  transfers**) = existing ERC-20 / asset transfer feed.
- **Data source**: Etherscan V2 `module=account&action=txlist` (and `txlistinternal`
  if internal txs are in scope — default MVP: normal txlist only). New port
  `AccountTransactionsPort` + adapter + cost hint + circuit breaker alignment
  with existing Etherscan client patterns.

**Domain**: `AccountTx`, `AccountTxPage`, cursor for pagination.

**Tests**: Two fixtures per adapter method (happy + error); functional tests for
merge ordering and empty state; BDD scenario on `address_detail` feature.

---

### Slice E — Incremental rendering (Address overview, Tx overview)

**Goals**

- **Address**: Emit `AddressOverview` as soon as the overview RPC returns;
  do not block the UI on `tokio::join!` with transfers + portfolio if that delays
  first paint — send updates on each completed branch (order documented in
  plan/16 feed section).
- **Transaction**: If receipt / trace / simulation slow the first full paint,
  emit partial `TxDetail` view models on the existing `tx_feed` channels (or split
  channels) so Overview tab can render hash, status, gas, **then** enrich logs.

**Tests**: Functional tests asserting order of received states (can use channels
in test harness).

---

### Slice F — Transaction detail: scroll, header border, raw copy

**Goals**

- (a) Same incremental story as Slice E for tx overview.
- (b) Header border uses muted when tabs/body focused (Slice A helper).
- (c) **Logs → Decoded** and **Logs → Raw** (and any nested scroll areas that
  overflow) scroll with ↑/↓ when that panel has focus.
- (d) Clipboard copy for a raw log field copies **value only**, not `label: value`.

**Tests**: Functional / snapshot on scroll offset; clipboard sink assertion for
raw row copy text.

---

### Slice G — Address as ERC-20 + Contract / Impl sub-tabs

**Goals**

- (a) Token **Overview** navigable values + clipboard (`FieldCursor` parity with
  EOA overview).
- (b) **ABI** sub-tab: ↑/↓ scroll long ABI text; `y` copies **full ABI** when ABI
  panel focused.
- (c) **Up** from ABI returns focus to **sub-tab strip** (same hierarchy as plan/16).
- (d) **Contract → Events**: parse / display event entries (ABI-driven decode or
  structured raw); ensure **Up** / hierarchy promotes from Events body to sub-tabs.
- (e) **Storage → Slot**: blinking caret on slot numeric input when focused.
- (f) **Read / Contract (and Impl)**: when the selected function has **no
  parameters**, hide the Arguments section and give **Result** more vertical space.
- (g) **Impl → Source → file**: source `Paragraph` supports scroll like other code
  panes.

**Tests**: Functional tests per sub-tab for focus promotion and scroll bounds;
BDD touch one happy path per bug class.

---

### Slice H — Navigation audit (cross-cutting)

**Goals**

- Systematic pass: every focus layer on unified address + tx detail screens must
  allow return to parent strip; every user-visible scalar must be in the
  navigable-field list or list row with `y` / `Enter` contract from plan/17.
- Document a checklist in this file appendix as items are verified.

## 4. Open questions

- **Portfolio upper bound**: Unlimited metadata fan-out is expensive; prefer
  pagination with explicit "load more" vs high hard cap — confirm CU budget.
- **txlist vs internal**: MVP may exclude internal-only txs; state clearly in UI.
- **Breadcrumb length**: Truncate middle segments on narrow terminals (reuse narrow
  layout threshold patterns from TUI rules).

## 5. Appendix — Verification checklist (fill during Slice H)

- [ ] Home → Address → Tx → back: breadcrumb matches stack.
- [ ] Address EOA: MainTabs → Subtabs → Overview fields → back to Subtabs.
- [ ] Address token: Token/Overview fields copy + navigate.
- [ ] Contract ABI: scroll, copy full, Up to sub-tabs.
- [ ] Contract Events: parsed rows, Up to sub-tabs.
- [ ] Tx logs decoded/raw: scroll + copy semantics.
