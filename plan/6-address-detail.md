# 6 — Address Detail

> Status: merged into [plan/16-unified-address-detail.md](16-unified-address-detail.md) (April 2026).
> Kept here as history for the feeds and ports that powered each tab.

Status: **done** — MVP Overview + Transactions + Tokens tabs are
live, the Contract tab appears dynamically when the loaded address
has bytecode, and the April-2026 follow-up (plan/15-backlog.md
§8.7) added reverse-ENS, `Y` copy, `e` CSV export, and portfolio
USD totals + chart. Activity classification and
approvals-with-revoke stay deferred; see section 13.

Account dossier. Active tabs in MVP: Overview, Transactions, Tokens, Activity, and
Contract (only when the address has code). The NFTs tab is documented in section 11
as future work but is not delivered in MVP.

## 1. Purpose and user goals

- Single screen that summarises a wallet: balance, ENS name, label, nonce.
- Unified transaction history across categories (external, internal, ERC-20, ERC-721,
ERC-1155).
- Full token portfolio with prices.
- Quick jump to the Contract view when the address has bytecode.

## 2. Layout

```
+-- Breadcrumb ----------------------------------------------------+
| ... > Address 0xd8dA... (vitalik.eth)                            |
+-- Header --------------------------------------------------------+
| 0xd8dA...6cAB   ENS: vitalik.eth   Label: "EOA"                  |
| Balance  523.14 ETH  ($1.82M)      Nonce  1,243                  |
| First seen 2015-08-07       Last seen 3m ago                     |
+-- Tabs ----------------------------------------------------------+
| [Overview] Transactions Tokens Activity Contract                 |
+-- Content -------------------------------------------------------+
|  ...                                                             |
+-- Status bar ----------------------------------------------------+
| Tab tabs  y copy address  e export CSV                           |
+------------------------------------------------------------------+
```

Tabs:

- **Overview**: balance panel + top 5 tokens (by USD) + last 5 activity events.
- **Transactions**: unified paginated list with a category filter on top (all /
external / internal / ERC-20 / ERC-721 / ERC-1155).
- **Tokens**: full ERC-20 portfolio with `symbol`, `balance`, `price`, `value`.
- **Activity**: timeline of higher-level events (approvals, swaps, mints); derived
locally from the unified transfers stream with a small categorisation rule set.
- **Contract**: only rendered when `eth_getCode` is non-empty; redirects to
`7-contract-detail.md` rendered inline as a tab.

## 3. Keybindings


| Key     | Action                                     |
| ------- | ------------------------------------------ |
| `Tab`   | Next tab                                   |
| `Enter` | Open selected tx / token / contract        |
| `e`     | Export currently-filtered transfers to CSV |
| `y`     | Copy address                               |
| `Y`     | Copy ENS name if available                 |
| `f`     | Open category filter modal                 |


## 4. Use cases

### 4.1 `LoadAddressOverview`

- **Input**: `Address`, chain.
- **Output**: `AddressOverview { balance, nonce, is_contract, ens_name, label, first_seen_at, last_seen_at }`.
- **Ports**: `AddressReaderPort`, `EnsResolverPort::reverse`, `LabelPort`,
`TransfersPort::first_and_last(addr)`.

### 4.2 `LoadAddressTransfers`

- **Input**: address, chain, category filter, pagination cursor.
- **Output**: page of `TransferEvent`.
- **Ports**: `TransfersPort::get_for_address(addr, categories, cursor)`.
- **Behaviour**: translates to `alchemy_getAssetTransfers` with the right
`category` list. Categories are mapped: internal -> only Ethereum mainnet and
Polygon mainnet (per Alchemy docs); others return `FeatureUnavailable`.

### 4.3 `LoadAddressPortfolio`

- **Input**: address, chain.
- **Output**: `Vec<TokenHolding>` sorted by USD value desc.
- **Ports**: `PortfolioPort::get_token_balances(addr)`, `PricesPort::get_bulk(addrs)`.
- **Behaviour**: one call to Portfolio API returns balances and metadata; a second
call to Prices API fills USD values.

### 4.4 `LoadAddressActivity`

- **Input**: address, chain.
- **Output**: timeline of `ActivityEvent`.
- **Ports**: `TransfersPort`, `ContractSourcePort` (for decoding approvals and swaps).
- **Behaviour**: fetches the unified transfers page, classifies each event into one
of (Send, Receive, Approval, Swap, Mint, Burn, ContractCreation, Other) based on
decoded method and log topics.

## 5. Ports required

- `AddressReaderPort`: `balance`, `nonce`, `code`.
- `TransfersPort`: `get_for_address`, `first_and_last`.
- `PortfolioPort`: `get_token_balances`.
- `PricesPort`: `get_bulk`.
- `LabelPort`: `label_for`.
- `EnsResolverPort`: `forward`, `reverse`.

## 6. Data sources

- Alchemy: `eth_getBalance`, `eth_getTransactionCount`, `eth_getCode`,
`alchemy_getAssetTransfers`, Portfolio `getTokenBalancesByAddress`,
Prices `getTokenPricesByAddress`.
- Etherscan V2: labels.
- ENS via `eth_call` on the ENS Registry (forward and reverse).

## 7. BDD scenarios (`tests/e2e/features/address_detail.feature`)

```gherkin
Feature: Address detail

  Background:
    Given the active chain is "ethereum"

  Scenario: EOA overview shows balance and ENS name
    Given the stub resolves "0xd8dA..." to ENS "vitalik.eth"
    When the user opens AddressDetail for "0xd8dA..."
    Then the header shows "vitalik.eth"
    And the balance is the value returned by the stub

  Scenario: Contract tab appears when code is non-empty
    Given "eth_getCode" returns bytecode for "0xa0b8..."
    When the user opens AddressDetail for "0xa0b8..."
    Then the Contract tab is visible
    And pressing Tab cycles to it

  Scenario: Labeled address shows tag
    Given the Etherscan label for "0x1111..." is "Uniswap V3 Router"
    When the user opens AddressDetail for "0x1111..."
    Then the header shows the tag

  Scenario: Transactions tab paginates and filters
    Given the stub returns 2 pages of transfers
    When the user opens the Transactions tab
    And sets the filter to "erc20"
    Then only ERC-20 transfers are shown
    And scrolling past the end loads the next page

  Scenario: Empty portfolio degrades gracefully
    Given the portfolio API returns an empty list
    When the user opens the Tokens tab
    Then an empty-state message is shown

  Scenario: Rate-limit retry
    Given the transfers endpoint returns 429 twice then succeeds
    When the user opens the Transactions tab
    Then the list is eventually populated
    And the retry count is observable via the status bar
```

## 8. Functional tests

- `LoadAddressOverview`: EOA vs contract; missing ENS; missing label; both missing.
- `LoadAddressTransfers`: category filter translation; pagination cursor handling;
internal category on unsupported chain returns `FeatureUnavailable`.
- `LoadAddressPortfolio`: empty list; price API partial failure (USD value becomes
`None` but balance still shown).
- `LoadAddressActivity`: classification rules per event type.

## 9. Fixtures

- `rpc__eth_getBalance__0xd8da.json`
- `rpc__eth_getTransactionCount__0xd8da.json`
- `rpc__eth_getCode__0xa0b8_contract.json`
- `rpc__eth_getCode__0xd8da_eoa.json`
- `rpc__alchemy_getAssetTransfers__0xd8da_page1.json`
- `rpc__alchemy_getAssetTransfers__0xd8da_page2.json`
- `portfolio__getTokenBalancesByAddress__0xd8da.json`
- `prices__getTokenPricesByAddress__bulk.json`
- `etherscan__label__0x1111_uniswap_router.json`
- `ens__reverse__0xd8da_vitalik_eth.json`

## 10. Open questions

- Do we merge the Transactions tab with the Activity tab? No: keep raw transfers
separate from classified events; the raw view is more useful for debugging, the
classified one for reading.

## 11. Deferred (not MVP)

- NFTs tab: ownership grid, collections, floor prices, rarity. Will live in a future
`plan/11-nfts.md`.
- WONT DO: Approvals panel with "revoke" action.
- Charts of historical balance.

**Shipped (April 2026, branch `probe/8.7-address-detail-followups`,
`plan/15-backlog.md` §8.7).** The four non-WONT-DO follow-ups of
this screen landed in a single slice:

- `Y` copies the ENS name when the loaded overview has one and falls
  back to the hex address otherwise. Pattern mirrors
  `BlockDetailScreen::last_copied_value` (plan/3 §12.1 / §8.4).
- Reverse ENS resolution flows through a real
  `AlchemyEnsResolver::reverse` implementation: namehash
  `<lower_hex>.addr.reverse`, `resolver(node)` on the ENS Registry,
  `name(node)` on the resolver, and a forward-resolve confirmation
  per `.cursor/rules/external-apis.mdc`. Results are cached with a
  5-minute TTL via `CachedEnsResolver`, a thin decorator over
  `EnsResolverPort` backed by the shared `TtlCache`. The
  `load_address_overview` use case now composes
  `AddressReaderPort` with `EnsResolverPort` so the Overview
  shows the reverse name without a second round-trip from the UI.
- `e` copies the currently-active tab to the clipboard sink as a
  CSV blob (Transactions, Tokens, Portfolio-with-prices, or a
  minimal Overview summary). No filesystem I/O is introduced; the
  body lands in `last_copied_value` next to the `y` / `Y`
  bindings so tests can assert on it deterministically.
- Portfolio USD totals + top-5 holdings chart: the Tokens tab
  gains a header row (`Σ USD` + "(N tokens not priced)" badge
  derived from `PriceLookup::Unsupported` / `Pending`) and a
  compact bar chart over the top-5 holdings by USD value. The
  USD column joins the existing list rows when a holding has a
  `PriceLookup::Available(_)` result.

## 12. Implementation plan

Three slices; only the Overview tab lands in this pass. The richer
tabs (Transactions, Tokens, Activity, Contract, NFTs) wait for the
Transfers / Portfolio / Prices / Etherscan adapters that are not in
the codebase yet.

### 12.1 Slice A — domain + port + use case + stubs

Domain additions (`src/domain/`):

- `address.rs` gains `AddressOverview { chain, address, balance, nonce, kind, ens_name }`. `kind` reuses the existing `AddressKind` enum;
`ens_name` stays `None` in MVP until reverse-ENS lookup ships.

Port (`src/application/ports/address_reader.rs`):

```rust
pub trait AddressReaderPort: Send + Sync {
    async fn get(&self, address: Address, chain: Chain)
        -> Result<Option<AddressOverview>, DomainError>;
}
```

Use case `load_address_overview` is a passthrough that maps `Ok(None)`
to `DomainError::NotFound`.

Stub `StubAddressReaderPort` with `insert(overview)` keyed by address.

Functional tests `tests/functional/load_address_overview.rs`:

- happy path for an EOA,
- happy path for a contract (kind = Contract, ens = None),
- missing address returns `NotFound`.

### 12.2 Slice B — Alchemy adapter

`src/adapters/rpc/address_reader.rs` issues `eth_getBalance`,
`eth_getTransactionCount` and `eth_getCode` in parallel via
`tokio::join!`, then assembles the `AddressOverview`. Reverse-ENS
stays deferred.

Tests `tests/functional/alchemy_address_reader.rs` use wiremock with
three canned responses per scenario. Fixtures:

- `rpc__eth_getBalance__0xd8da.json`
- `rpc__eth_getTransactionCount__0xd8da.json`
- `rpc__eth_getCode__0xd8da_eoa.json`
- `rpc__eth_getCode__0xa0b8_contract.json`

### 12.3 Slice C — UI + wiring + BDD

- `src/adapters/ui/address_detail.rs` with an Overview-only screen
that renders address, kind label, ENS (when present), balance in
wei/gwei/ether, and nonce.
- `src/infra/address_feed.rs` — same shape as `block_feed.rs`:
channel pair, spawn helper that owns an `AddressReaderPort`.
- `infra::run` and the search detail factory in `tests/e2e/steps/search.rs`
route `ResolvedEntity::Address` to the new screen (previously a
`DetailPlaceholderScreen`).
- BDD `tests/e2e/features/address_detail.feature`:
  - Open address detail for an EOA and assert the header kind says EOA.
  - Open address detail for a contract and assert the header kind
  says Contract.

Acceptance: plan flipped to `done (MVP)`, clippy clean, all tests
green.

## 12.4 Expanded slices (delivered in this iteration)

Three commits extend the screen with the remaining MVP-scoped tabs.
Activity tab and CSV/approvals UX stay deferred (section 13).

### 12.4.1 Commit 1 — Transactions tab

Domain (`src/domain/transfers.rs`):

- `TransferCategory { External, Internal, Erc20, Erc721, Erc1155 }`.
- `TransferAsset` enum with `Native { symbol }`, `Erc20 { contract, symbol, decimals }`, `Nft { contract, kind: NftKind, token_id }`.
- `TransferEvent { chain, block_number, tx_hash, from, to: Option, asset, value: Wei, category }`.
- `TransferPage { events, next_cursor: Option<String> }`.

Port `TransfersPort::get_for_address(addr, chain, cursor) -> TransferPage`. The adapter issues two parallel
`alchemy_getAssetTransfers` calls (one with `fromAddress=addr`,
one with `toAddress=addr`), merges results by block number
descending and caps at a sensible maximum. Cursor aggregates both
pageKeys; MVP ships without "load more" but the cursor is already
modelled so it can be wired later.

Use case `load_address_transfers` is a thin delegator that maps
`Ok(None)` to `NotFound`.

UI changes on `AddressDetailScreen`:

- Tab cycle Overview -> Transactions (-> Tokens -> Contract after
commits 2 and 3) rendered through `ratatui::widgets::Tabs` so the
bar stays stable (mirrors the TxDetail fix).
- Transactions tab shows a selectable list: `[category] from -> to value (asset)  block  #idx`. Up/Down move the selection,
PageUp/PageDown page by 10, Enter opens the referenced
`TxDetailScreen` through the same open-tx factory the BlockDetail
already uses.

BDD:

- Transactions tab renders the returned transfers.
- Enter on a transfer opens a TxDetail screen.

### 12.4.2 Commit 2 — Tokens tab

Domain: `TokenHolding { metadata: TokenMetadata, balance: Wei }`.
USD pricing stays deferred.

Port `PortfolioPort::get_token_balances(addr, chain) -> Vec<TokenHolding>`, backed by Alchemy's `alchemy_getTokenBalances`
plus a follow-up `alchemy_getTokenMetadata` for every non-zero
holding. The adapter caps at the top 20 non-zero holdings to keep
the metadata fan-out bounded.

UI: Tokens tab lists `symbol  balance  (contract)`. Enter opens a
TokenDetail screen via a new open_token factory.

BDD:

- Tokens tab renders the portfolio.
- Empty portfolio shows an empty-state message.

### 12.4.3 Commit 3 — Contract tab + overview enrichment

When `AddressOverview.kind == Contract` the tab bar gains a
Contract entry. Pressing Enter on that tab pushes a
`ContractDetailScreen` for the same address, reusing the
existing plan-7 infrastructure. EOAs simply do not see the tab.

Overview body grows a short status line summarising the loaded
data (e.g. `Txs loaded: 42   Tokens loaded: 7`) so the Overview
is not just a static four-line block anymore.

### 12.4.4 Commit 4 — Inline Token tab for ERC-20 contracts

When the address turns out to be an ERC-20 contract the tab bar
gains a new `Token` entry (sits between `Tokens` and `Contract`).
Unlike the Contract tab, which is just a shortcut to
`ContractDetailScreen`, the Token tab renders the TokenDetail
content **inline**: the same symbol / name / decimals / supply /
price / market-cap block plus a compact price mini-chart for the
D1 window (24h / 1h granularity).

**Pessimistic detection.** The probe is gated inside
`src/infra/address_feed.rs`: only when the `AddressOverview` from
the reader reports `kind == Contract` do we call
`TokenReaderPort::get`. EOAs pay zero extra RPC calls.
Non-ERC20 contracts pay exactly one `alchemy_getTokenMetadata`
RTT whose negative result flips the internal
`TokenProbeState::NotToken`, and the tab stays hidden.

```text
ov_res.kind == Contract
  ├─ token_reader.get(addr) = Some(ov)
  │    ├─ prices.get_single(addr)
  │    └─ prices.get_history(addr, D1)
  ├─ token_reader.get(addr) = None    → tab stays hidden
  └─ token_reader.get(addr) = Err     → tab stays hidden
```

**Channels.** `AddressFeed` / `AddressFeedSender` gain three new
unbounded channels: `token_overview`, `token_price`, `token_series`.
The screen drains them in `tick`.

**State machine.** Inside `AddressDetailScreen` a new tri-state
`TokenProbeState::{Unknown, NotToken, IsToken(TokenOverview)}`
gates the tab's visibility. Price and historical series live in
parallel `Option` fields with a "probed but no data" flag for the
price column.

**Key bindings.** Tab cycles tabs as before. On the Token tab, `o`
(or Enter) pushes the full `TokenDetailScreen` for the same
address through the `OpenTokenFactory`.

**Shared render helper.** `adapters/ui/token_detail.rs` exports
`pub(crate) fn render_inline_token_panel(frame, area, overview, price, price_missing, series, window)` used by both the full
TokenDetailScreen and the inline AddressDetail Token tab, so the
two views stay visually consistent.

**BDD.** `tests/e2e/features/address_detail.feature` gains three
scenarios: ERC-20 contract shows the Token tab and its inline
content; non-ERC20 contract and EOA do not.

**Functional.** The existing `search_feed_token_probe.rs`
functional tests already cover the contract-gated probing pattern;
the address feed version is covered end-to-end by BDD using the
`StubTokenReaderPort::call_count` counter for implicit assertions
on pessimistic behaviour (via scenarios that never reach
`TokenPort::Some`).

## 13. Won't do

- Activity classification (Send / Receive / Approval / Swap / Mint
/ Burn / ContractCreation / Other) — needs the full ABI decoder
and heuristics over the transfers + logs stream.
- Category filter modal (`f` binding) — the Transactions tab
currently merges every category; filtering UI lands later.
- Approvals list with "revoke" action.
- Historical balance chart.
- NFTs tab.

**Promoted in April 2026 (see §11 "Shipped" block):**

- Reverse-ENS resolution (`Y` copy + Overview badge).
- CSV export on `e` (active-tab snapshot into the clipboard sink).
- Portfolio USD totals + top-5 holdings bar chart on the Tokens tab
(the single-token mini-chart from §12.4.4 stays as before).

