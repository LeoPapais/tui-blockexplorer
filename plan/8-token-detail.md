# 8 — Token Detail

> Status: merged into [plan/16-unified-address-detail.md](16-unified-address-detail.md) (April 2026).
> Kept here as history for the feeds and ports that powered each tab.

Status: **in progress** — Overview tab (metadata + totalSupply) is
live and covered by BDD. Slice 12.4 extends it with:

- spot price + market cap on the Overview header,
- a Transfers tab backed by `alchemy_getAssetTransfers` filtered by
`contractAddresses`,
- a Chart tab that draws a price sparkline over one of three
selectable windows (`1d` / `1m` / `1y`) using the Alchemy Prices
API.

The Holders tab is documented as deferred (section 11).

Page for an ERC-20 token. Tabs in MVP: Overview, Transfers, Chart. The Holders tab is
documented as deferred (section 11).

## 1. Purpose and user goals

- Quick factual overview of a token (name, symbol, decimals, supply, price, market
cap).
- Live feed of recent transfers of the token.
- Historical price chart.

## 2. Layout

```
+-- Breadcrumb ----------------------------------------------------+
| ... > Token USDC                                                 |
+-- Header --------------------------------------------------------+
| USDC  "USD Coin"   ERC-20                                        |
| Address  0xA0b8...   Decimals 6                                  |
| Price  $1.0001     Supply  35.2B     Market cap  $35.2B          |
+-- Tabs ----------------------------------------------------------+
| [Overview] Transfers Chart                                       |
+-- Content -------------------------------------------------------+
|  ...                                                             |
+-- Status bar ----------------------------------------------------+
| Tab tabs  y copy address                                         |
+------------------------------------------------------------------+
```

Tabs:

- **Overview**: header fields plus contract address (links to ContractDetail) and a
one-line description from the metadata if any.
- **Transfers**: recent transfers for this token (`block`, `age`, `from`, `to`,
`amount`, `tx hash`).
- **Chart**: sparkline of the price over the selected window (24h default, 7d, 30d,
90d).

## 3. Keybindings


| Key      | Action                             |
| -------- | ---------------------------------- |
| `Tab`    | Next tab                           |
| `Enter`  | On a transfer row open TxDetail    |
| `y`      | Copy contract address              |
| `1`..`3` | Switch chart window (1d / 1m / 1y) |


## 4. Use cases

### 4.1 `LoadTokenOverview`

- **Input**: `Address`, chain.
- **Output**: `TokenOverview { metadata, total_supply, price, market_cap, address }`.
- **Ports**: `TokenMetadataPort::get`, `ContractReaderPort::call` (for
`totalSupply()`), `PricesPort::get_single`.
- **Behaviour**: if metadata fails, mark the token as "unsupported" and let the
screen render a friendly empty state.

### 4.2 `LoadTokenTransfers`

- **Input**: `Address`, chain, pagination cursor.
- **Output**: page of `TransferEvent` filtered by `contractAddresses=[addr]`.
- **Ports**: `TransfersPort::get_for_contract`.

### 4.3 `LoadTokenPriceHistory`

- **Input**: `Address`, chain, window (`PriceWindow::D1`, `M1`, `Y1`).
  - `D1` → last 24h sampled at 1-hour granularity (≈ 24 points).
  - `M1` → last 30 days sampled at 1-day granularity (≈ 30 points).
  - `Y1` → last 365 days sampled at 1-week granularity (≈ 52 points).
- **Output**: `PriceSeries { points: Vec<PricePoint>, window, currency }`.
- **Ports**: `PricesPort::get_history(addr, chain, window)`.
- **Behaviour**: the adapter maps the window into the Alchemy Prices
API `interval` + `startTime/endTime` combo and returns points in
chronological order. Empty history degrades to a flat "no data"
state in the UI.

### 4.4 `LoadTokenPrice`

- **Input**: `Address`, chain.
- **Output**: `Option<TokenPrice>` — spot price in USD with the
timestamp returned by Alchemy.
- **Ports**: `PricesPort::get_single(addr, chain)`.
- **Behaviour**: `Ok(None)` maps to `DomainError::NotFound` at the
application boundary. Market cap is derived by the UI from
`price × totalSupply / 10^decimals` when both are available.

## 5. Ports required

- `TokenMetadataPort`: `get(addr, chain)`.
- `ContractReaderPort`: `call` (already defined in `7-contract-detail.md`).
- `PricesPort`: `get_single`, `get_history`.
- `TransfersPort`: `get_for_contract`.

## 6. Data sources

- Alchemy JSON-RPC:
  - `alchemy_getTokenMetadata` (metadata),
  - `eth_call` against the `totalSupply()` selector (supply),
  - `alchemy_getAssetTransfers` with `contractAddresses=[addr]` for
  the Transfers tab.
- Alchemy Prices API
(`https://api.g.alchemy.com/prices/v1/{api_key}/tokens/...`):
  - `POST /tokens/by-address` (single spot price, request body:
  `{addresses: [{network, address}], currencies: ["usd"]}`).
  - `POST /tokens/historical` (history, request body:
  `{network, address, startTime, endTime, interval}`). Intervals
  used: `1h` (window `D1`), `1d` (window `M1`), `1w` (window
  `Y1`).

The Prices API lives on a different host than the JSON-RPC adapter,
so it owns a dedicated `AlchemyPricesClient` at
`src/adapters/prices/client.rs`. The JSON-RPC `RpcClient` is
deliberately left untouched.

## 7. BDD scenarios (`tests/e2e/features/token_detail.feature`)

```gherkin
Feature: Token detail

  Background:
    Given the user is on Home
    And the active chain is "ethereum"

  Scenario: Standard ERC-20 overview with price
    Given the token reader knows "0xa0b8..." as "USDC" / "USD Coin" decimals 6 supply 35188571170816
    And the prices stub returns $1.0001 for "0xa0b8..."
    When the user opens TokenDetail for "0xa0b8..."
    Then the Overview shows price "$1.0001"
    And market cap shows "$35,188,571.17"

  Scenario: Token without price degrades gracefully
    Given the token reader knows "0xa0b8..." as "USDC" / "USD Coin" decimals 6 supply 1000000000000
    And the prices stub has no data for "0xa0b8..."
    When the user opens TokenDetail for "0xa0b8..."
    Then the Overview price cell shows "-"
    And the Overview market cap shows "-"

  Scenario: Historical chart renders a selected window
    Given the prices stub returns 30 points for the 1m window on "0xa0b8..."
    When the user opens TokenDetail for "0xa0b8..."
    And the user switches to the Chart tab
    And the user selects window "1m"
    Then the chart shows 30 points

  Scenario: Transfers tab opens TxDetail on Enter
    Given the transfers stub returns 2 transfers for the token "0xa0b8..."
    When the user opens TokenDetail for "0xa0b8..."
    And the user switches to the Transfers tab
    And the user presses Enter on the first transfer row
    Then a TxDetail screen is pushed for that tx hash
```

## 8. Functional tests

- `LoadTokenOverview`: full happy path; missing price; missing supply; missing
metadata.
- `LoadTokenTransfers`: pagination cursor; filter by `contractAddresses`; error
maps to domain error.
- `LoadTokenPriceHistory`: each window size; partial data (fewer points than
expected); empty history.

## 9. Fixtures

- `rpc__alchemy_getTokenMetadata__usdc.json`
- `rpc__alchemy_getTokenMetadata__missing.json`
- `rpc__eth_call__totalSupply_usdc.json`
- `rpc__alchemy_getAssetTransfers__usdc_recent.json`
- `prices__by_address__usdc.json`
- `prices__by_address__unknown.json`
- `prices__historical__usdc_1d.json` (1h granularity)
- `prices__historical__usdc_1m.json` (1d granularity)
- `prices__historical__usdc_1y.json` (1w granularity)

## 10. Open questions

- Should we also display the contract's ABI link in the overview? Yes, navigate to
`ContractDetail` with `o` (open related menu).

## 11. Deferred

- **WONT-DO**: Holders tab (top-N holders, distribution chart). Alchemy does
not expose this directly; deprioritised for MVP. Tracked in
`plan/15-backlog.md` §8.9.
- **WONT-DO**: Multi-chain aggregated view (same token across chains).
Tracked in `plan/15-backlog.md` §8.9.
- **Shipped (13)**: Live price streaming. The dispatcher now subscribes to a
`TokenPriceStreamPort` that polls `PricesPort::get_single` every 15 s and
appends points to a rolling 60-sample window on the Chart tab without
requiring the user to reopen the screen. See §13.1.
- **Shipped (13)**: Unsupported-token empty state. `TokenMetadata::is_incomplete`
now short-circuits the Overview into a dedicated card with a
"View as Contract" shortcut on `c` when `symbol` / `decimals` are missing.
See §13.2.

## 12. Implementation plan

Three slices. MVP lands the Overview tab with metadata + totalSupply;
price / market cap / Transfers / Chart require new adapters we have
not written (Prices API, Transfers API) and stay deferred in 12.4.

### 12.1 Slice A — domain + port + use case + stubs

Domain (`src/domain/token.rs`): existing `TokenMetadata` gains a
sibling struct

```rust
pub struct TokenOverview {
    pub metadata: TokenMetadata,
    pub total_supply: u128,
}
```

Port (`src/application/ports/token_reader.rs`):

```rust
pub trait TokenReaderPort: Send + Sync {
    async fn get(&self, address: Address, chain: Chain)
        -> Result<Option<TokenOverview>, DomainError>;
}
```

Use case `load_token_overview`: passthrough that maps `Ok(None)` to
`DomainError::NotFound`.

Stub `StubTokenReaderPort` + three functional tests (happy path,
missing token, truncated metadata).

### 12.2 Slice B — Alchemy adapter

`src/adapters/rpc/token_reader.rs` exposes `AlchemyTokenReader`:

- `alchemy_getTokenMetadata` → `{ name, symbol, decimals, logo }`;
missing decimals or symbol short-circuit to `Ok(None)` (the plan
describes this as "unsupported token").
- `eth_call` with the ERC-20 `totalSupply()` selector `0x18160ddd`
at the provided address; the 32-byte return decodes as a u128
(we truncate anything above the low 128 bits to keep the shape
lean — tokens with supplies above 2^128 are real but rare; the
UI renders the raw number without unit scaling anyway).
- Both calls via `tokio::join!`.

Fixtures + wiremock tests cover happy path and missing-metadata.

### 12.3 Slice C — UI + wiring + BDD

- `src/adapters/ui/token_detail.rs` — `TokenDetailScreen` with the
Overview tab rendering name, symbol, decimals, contract address
and raw `totalSupply()` value. Transfers and Chart tabs show a
"deferred" banner.
- `src/infra/token_feed.rs` — `spawn` helper mirroring the other
feeds.
- Search detail factory now routes `ResolvedEntity::Token` to the
new screen. Direct-open step for BDD pushes it without going
through Search.
- BDD `tests/e2e/features/token_detail.feature`:
  - Open a known token and assert the Overview shows the stub
  symbol + totalSupply.
  - Open an address with no metadata and assert the screen renders
  the "unsupported" fallback.

### 12.4 Slice D — Prices, Transfers, Chart

Domain:

```rust
pub struct TokenPrice {
    pub currency: String,           // "usd" for MVP
    pub value: f64,
    pub as_of: UnixTimestamp,
}

pub enum PriceWindow { D1, M1, Y1 }

pub struct PricePoint {
    pub at: UnixTimestamp,
    pub value: f64,
}

pub struct PriceSeries {
    pub window: PriceWindow,
    pub currency: String,
    pub points: Vec<PricePoint>,
}
```

`TokenOverview` gains an optional `price: Option<TokenPrice>` field
so the overview screen can reuse the existing reader pipeline when
the Prices API is unreachable.

Ports:

```rust
pub trait PricesPort: Send + Sync {
    async fn get_single(&self, addr: Address, chain: Chain)
        -> Result<Option<TokenPrice>, DomainError>;
    async fn get_history(&self, addr: Address, chain: Chain, window: PriceWindow)
        -> Result<PriceSeries, DomainError>;
}

pub trait TransfersPort {
    // ... existing get_for_address ...
    async fn get_for_contract(
        &self,
        contract: Address,
        chain: Chain,
        cursor: Option<TransferCursor>,
    ) -> Result<TransferPage, DomainError>;
}
```

Use cases: `load_token_price`, `load_token_price_history`,
`load_token_transfers`.

Adapter: `src/adapters/prices/` (new module) owns an
`AlchemyPricesClient` that speaks REST against
`https://api.g.alchemy.com/prices/v1/{api_key}`. The JSON-RPC client
stays single-purpose for consistency with the existing layering.

`AlchemyTransfers::get_for_contract` reuses the same JSON-RPC
client: it issues a single `alchemy_getAssetTransfers` with
`contractAddresses=[addr]` and `category=["erc20"]` (other
categories don't make sense for a contract-filtered query).

UI:

- `TokenDetailScreen` is reshaped around `TokenTab::{Overview, Transfers, Chart}`.
- The Chart tab uses `ratatui::widgets::Chart` with a single dataset
(x = point index, y = price in USD). Window switching lives on
`1` / `2` / `3` keys; `Tab` / `Shift+Tab` cycles tabs.
- The Overview adds a "Price" and a "Market cap" line. The latter
is formatted using the classic US grouping; when either price or
supply/decimals is missing it falls back to `-`.

### 12.5 Won't Do

- Holders tab (see section 11).
- Fiat currencies other than USD.

## 13. Shipped follow-ups (from `plan/15-backlog.md` §8.9)

### 13.1 Live price streaming

Until this slice the Chart tab only refetched a window when the user
switched between `1` / `2` / `3`; the current window stayed frozen.

**Port.** `src/application/ports/token_price_stream.rs` introduces

```rust
pub trait TokenPriceStreamPort: Send + Sync {
    fn subscribe(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl Future<Output = Result<UnboundedReceiver<PriceLookup>, DomainError>> + Send;
}
```

Semantics match `NewHeadsStreamPort` (`plan/1-home.md` §12.2): dropping the
sender side signals that the upstream connection went away; the dispatcher
then stops appending new samples.

**Adapter.** `src/adapters/prices/stream.rs` provides
`PollingTokenPriceStream<P>` that wraps any `PricesPort` and re-issues
`get_single` every `interval` (15 s default). The ticker lives in the
adapter task, not in any use case — consistent with the rule in
`.cursor/rules/tui.mdc` ("screens never do I/O in render").

**Stub.** `StubTokenPriceStreamPort` in `tests/support/stubs.rs` lets
scenarios push `PriceLookup` values on demand with `push(lookup)`.

**Feed + UI.** `src/infra/token_feed.rs` spawns one subscription per
incoming address and forwards every received lookup onto the existing
`price_tx` channel. The Chart tab maintains a rolling window of up to 60
samples (`PriceSeries::ROLLING_CAP`). Each streaming `Available` value
appends a point to the series for the active window (so switching to the
D1 window while the stream is live keeps appending). `Unsupported` /
`Pending` leave the series untouched but still update the Overview price
cell.

**Tests.**

- Functional: `tests/functional/token_price_stream.rs` drives the stub port
  end-to-end and asserts the screen observes a second price sample without
  a window switch.
- BDD: `tests/e2e/features/token_detail.feature` "Token price updates every
  tick without reopening the screen".

### 13.2 Unsupported-token empty state

`TokenMetadata::is_incomplete()` returns `true` when either `symbol` is
empty or both `name.is_empty()` and `decimals == 0` (the two ways
`alchemy_getTokenMetadata` can degenerate on non-standard contracts).
`TokenOverview::is_incomplete` delegates to it.

When `is_incomplete()` returns `true` the Overview renders a dedicated
empty-state card with the copy

```
This address does not look like a standard ERC-20.
Decimals / symbol are missing from alchemy_getTokenMetadata.
[c] View as Contract   [Esc] back
```

Pressing `c` emits `Command::Push` with a `ContractDetailScreen` built
through the same `OpenContractFactory` pattern used on Address Detail
(`plan/6-address-detail.md` §12.4.4).

BDD scenario: "Non-standard token shows the incomplete badge and jumps to
Contract Detail on c".


