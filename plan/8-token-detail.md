# 8 — Token Detail

Status: **done** (MVP scope) — Overview tab with metadata + totalSupply
is live, both BDD scenarios in
`tests/e2e/features/token_detail.feature` are green. Search now
opens a real TokenDetailScreen for token candidates. Price, market
cap, Transfers tab, Chart tab and Holders tab all remain deferred
(section 12.4).

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

| Key     | Action                                       |
|---------|----------------------------------------------|
| `Tab`   | Next tab                                     |
| `Enter` | On a transfer row open TxDetail              |
| `y`     | Copy contract address                        |
| `1`..`4`| Switch chart window (24h / 7d / 30d / 90d)   |

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

- **Input**: `Address`, chain, window (`Window::H24`, `D7`, `D30`, `D90`).
- **Output**: `PriceSeries { points: Vec<(UnixTimestamp, f64)> }`.
- **Ports**: `PricesPort::get_history(addr, window)`.

## 5. Ports required

- `TokenMetadataPort`: `get(addr, chain)`.
- `ContractReaderPort`: `call` (already defined in `7-contract-detail.md`).
- `PricesPort`: `get_single`, `get_history`.
- `TransfersPort`: `get_for_contract`.

## 6. Data sources

- Alchemy: `alchemy_getTokenMetadata`, `eth_call totalSupply()`,
  `alchemy_getAssetTransfers` filtered by `contractAddresses`.
- Prices API: `getTokenPricesByAddress`, `getHistoricalTokenPrices`.

## 7. BDD scenarios (`tests/e2e/features/token_detail.feature`)

```gherkin
Feature: Token detail

  Background:
    Given the active chain is "ethereum"

  Scenario: Standard ERC-20 overview
    Given the stub knows token metadata and price for USDC
    When the user opens TokenDetail for USDC
    Then the header shows "USDC", decimals 6, price "$1.0001"
    And total supply is rendered using human units

  Scenario: Token without price degrades gracefully
    Given the Prices stub returns "not found" for the token
    When the user opens TokenDetail
    Then the price cell shows "-"
    And market cap shows "-"

  Scenario: Historical chart renders a selected window
    Given the Prices stub returns 30 points for the 30d window
    When the user opens the Chart tab and presses "3"
    Then a sparkline of 30 points is drawn

  Scenario: Non-ERC20 address shows "unsupported"
    Given token metadata returns no symbol and no decimals
    When the user opens TokenDetail
    Then the screen shows an "unsupported token" empty state

  Scenario: Transfers tab opens TxDetail on Enter
    Given the stub returns 2 transfers for the token
    When the user opens the Transfers tab and presses Enter on the first row
    Then the TxDetail screen is pushed with that tx hash
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
- `rpc__alchemy_getTokenMetadata__unknown.json`
- `rpc__eth_call__totalSupply_usdc.json`
- `rpc__alchemy_getAssetTransfers__usdc_recent.json`
- `prices__getTokenPricesByAddress__usdc.json`
- `prices__getTokenPricesByAddress__unknown.json`
- `prices__getHistoricalTokenPrices__usdc_30d.json`

## 10. Open questions

- Should we also display the contract's ABI link in the overview? Yes, navigate to
  `ContractDetail` with `o` (open related menu).

## 11. Deferred

- Holders tab (top N holders, distribution chart). Requires data not directly
  provided by Alchemy and is deprioritised for MVP.
- Multi-chain aggregated view (same token across chains) — future.

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

### 12.4 Deferred (next slices under this plan)

- Prices API adapter feeding the price / market-cap lines.
- Transfers API adapter feeding the Transfers tab.
- Historical prices for the Chart tab.
- Holders tab.
