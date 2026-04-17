# 6 — Address Detail

Status: **done** (MVP scope) — Overview tab is live, the two BDD
scenarios in `tests/e2e/features/address_detail.feature` are green
and Search opens a real AddressDetailScreen instead of the
placeholder. Transactions / Tokens / Activity / Contract / NFTs tabs
remain deferred (section 11).

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

| Key     | Action                                              |
|---------|-----------------------------------------------------|
| `Tab`   | Next tab                                            |
| `Enter` | Open selected tx / token / contract                 |
| `e`     | Export currently-filtered transfers to CSV          |
| `y`     | Copy address                                        |
| `Y`     | Copy ENS name if available                          |
| `f`     | Open category filter modal                          |

## 4. Use cases

### 4.1 `LoadAddressOverview`

- **Input**: `Address`, chain.
- **Output**: `AddressOverview { balance, nonce, is_contract, ens_name, label,
  first_seen_at, last_seen_at }`.
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
- Approvals panel with "revoke" action.
- Charts of historical balance.

## 12. Implementation plan

Three slices; only the Overview tab lands in this pass. The richer
tabs (Transactions, Tokens, Activity, Contract, NFTs) wait for the
Transfers / Portfolio / Prices / Etherscan adapters that are not in
the codebase yet.

### 12.1 Slice A — domain + port + use case + stubs

Domain additions (`src/domain/`):

- `address.rs` gains `AddressOverview { chain, address, balance, nonce,
  kind, ens_name }`. `kind` reuses the existing `AddressKind` enum;
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
