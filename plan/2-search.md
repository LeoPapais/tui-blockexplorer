# 2 — Universal Search

Status: **done** — all five BDD scenarios in
`tests/e2e/features/search.feature` are green along with the
functional coverage under `tests/functional/`.

With the block list and the transaction list removed, the Search screen is the only
way to reach detail screens for entities the user does not already have on screen. It
must be fast, resilient to ambiguous input, and produce a single, obvious next step.

## 1. Purpose and user goals

- Accept any blockchain identifier or human handle and resolve it to a concrete
  `ResolvedEntity`.
- Show ranked candidates when the input is ambiguous, with enough detail for the user
  to pick.
- Handle "not found" gracefully with actionable hints.

## 2. Layout

```
+-- Search ----------------------------------------------------+
| >  0x8f3c...                                                 |
+--------------------------------------------------------------+
|  Type detected: transaction hash                             |
|  Chain: ethereum                                             |
|                                                              |
|  Candidates:                                                 |
|   > [tx]      0x8f3c...  block 21345001  success             |
|     [block]   hash 0x8f3c... (same value, unlikely)          |
|                                                              |
+-- Status bar ------------------------------------------------+
| Enter open   Tab next candidate   Esc cancel                 |
+--------------------------------------------------------------+
```

Search opens as a modal over whatever screen is active. Once the user picks a
candidate, the modal closes and a new detail screen is pushed on the stack.

## 3. Input detection rules

Applied in this order. First match wins, but if a later rule is still plausible it is
offered as an additional candidate.

| Pattern                                   | Primary interpretation        |
|-------------------------------------------|-------------------------------|
| `^0x[0-9a-fA-F]{64}$`                     | Transaction hash              |
| `^0x[0-9a-fA-F]{64}$` (fallback)          | Block hash                    |
| `^0x[0-9a-fA-F]{40}$`                     | Address (EOA or contract)     |
| `^[0-9]+$`                                | Block number                  |
| `^.+\.eth$` (and other namespaces later)  | ENS name -> forward resolve   |
| `^[A-Z]{2,10}$`                           | Token ticker (local list)     |
| Any other non-empty string                | Token or contract name search |

Normalisation is applied before classification (see section 12.1):

- `trim_matches(|c| c.is_whitespace() || c == '"' || c == '\'')`.
- Lowercase every ASCII hex body (preserves ticker casing).
- Recognise Etherscan-style block-explorer URLs and extract the trailing
  path segment as the effective input (see section 12.2).

## 4. Use case: `ResolveQuery`

### 4.1 Signature

```
pub struct ResolveQuery { ... }

impl ResolveQuery {
  pub async fn run(&self, input: &str, chain: Chain)
    -> Result<Vec<ResolvedEntity>, DomainError>;
}

pub enum ResolvedEntity {
  Block { number: BlockNumber, hash: BlockHash },
  Tx { hash: TxHash, block: Option<BlockNumber> },
  Address { address: Address, is_contract: bool, ens_name: Option<String> },
  Token { address: Address, symbol: String, name: String },
  NotFound { reason: String },
}
```

### 4.2 Behaviour

1. Normalize: trim, lowercase hex, strip whitespace.
2. Classify against the table in section 3.
3. For each plausible classification, query the corresponding port in parallel.
4. Collect results, deduplicate, rank: exact type match first, ENS resolutions second,
   partial text matches last.
5. If the list is empty, return `vec![ResolvedEntity::NotFound { reason }]` with a
   human-readable reason (for example "No transaction and no block with hash
   0x8f3c... on ethereum").

### 4.3 Ports used

- `TxLookupPort::get(tx_hash, chain) -> Option<TxSummary>`
- `BlockLookupPort::get_by_hash(hash, chain) -> Option<BlockSummary>`
- `BlockLookupPort::get_by_number(n, chain) -> Option<BlockSummary>`
- `AddressLookupPort::classify(addr, chain) -> AddressKind` (EOA vs contract)
- `EnsResolverPort::forward(name, chain) -> Option<Address>`
- `TokenSearchPort::by_symbol(sym, chain) -> Vec<TokenMetadata>`
- `TokenSearchPort::by_name(text, chain) -> Vec<TokenMetadata>`

## 5. Data sources

- Tx / block lookups: Alchemy (`eth_getTransactionByHash`, `eth_getBlockByHash`,
  `eth_getBlockByNumber`).
- Contract classification: Alchemy (`eth_getCode`).
- ENS forward: `eth_call` on the ENS Public Resolver via Alchemy.
- Token search: Etherscan V2 (`getsourcecode` + local token list for popular symbols).

## 6. BDD scenarios (`tests/e2e/features/search.feature`)

```gherkin
Feature: Universal search

  Background:
    Given the user is on Home
    And the active chain is "ethereum"

  Scenario: Resolves a valid transaction hash
    When the user opens search with "0x8f3c...abcd"
    Then the primary candidate is "transaction"
    And pressing Enter pushes the TxDetail screen

  Scenario: Disambiguates between tx hash and block hash
    Given the hash "0xdead...beef" matches both a transaction and a block in the stub
    When the user opens search with "0xdead...beef"
    Then there are at least two candidates in the list
    And the first candidate is "transaction"

  Scenario: Resolves an ENS name
    Given "vitalik.eth" resolves to "0xd8dA..." in the stub
    When the user opens search with "vitalik.eth"
    Then the primary candidate is "address"
    And the candidate row shows the ENS name next to the address

  Scenario: Resolves a block number
    When the user opens search with "21345678"
    Then the primary candidate is "block"
    And pressing Enter pushes the BlockDetail screen

  Scenario: Not found with actionable hint
    Given the input "0x0000000000000000000000000000000000000000000000000000000000000001" matches nothing
    When the user opens search with that value
    Then the candidates list contains a single "not found" entry
    And the hint mentions the active chain name
```

## 7. Functional tests

- Classification table: parameterized test with the patterns from section 3.
- Parallel port calls: ensure a failure in one port does not block the others (test
  by stubbing one port with an artificial delay and another with an error).
- Ranking: exact type wins over ENS; ENS wins over text search.
- Normalization: uppercase hex, leading whitespace, "eth" lowercased handled.

## 8. Fixtures

- `rpc__eth_getTransactionByHash__found.json`
- `rpc__eth_getTransactionByHash__not_found.json`
- `rpc__eth_getBlockByHash__found.json`
- `rpc__eth_getBlockByHash__not_found.json`
- `rpc__eth_getBlockByNumber__found.json`
- `rpc__eth_getCode__eoa.json`
- `rpc__eth_getCode__contract.json`
- `ens__forward__vitalik_eth.json`
- `etherscan__getsourcecode__usdc.json`

## 9. Open questions

- Should we cache resolution results across opens of the modal? **Yes, 60 s
  TTL keyed by `(Chain, normalised_input)`. Shipped in section 12 via the
  new `TtlCache` adapter under `src/adapters/cache/memory.rs`.**
- Should we support pasting a full URL (for example an etherscan.io link)?
  **Yes. `classify_input` now recognises `https?://(www\.)?(etherscan.io|
  polygonscan.com|basescan.org|arbiscan.io|optimistic.etherscan.io|
  sepolia.etherscan.io)/(tx|address|block)/{value}` and unwraps the path
  segment before applying the classification table from section 3. See
  section 12.2.**

## 10. Implementation plan

Delivered in three slices, each its own commit with the full red->green loop.

### 10.1 Slice A — domain, ports, use case, stubs (no network, no UI)

New or extended domain types (`src/domain/`):

- `address.rs` — `pub struct Address([u8; 20])` with `from_hex_prefixed` and a
  lowercase canonical hex string formatter.
- `tx.rs` — `pub struct TxHash([u8; 32])` with the same fallible constructor.
- `block.rs` — add `pub struct BlockHash([u8; 32])` next to `BlockNumber`.
- `token.rs` — `pub struct TokenMetadata { address: Address, symbol: String,
  name: String, decimals: u8 }`.
- `search.rs` — `pub enum ResolvedEntity { Block, Tx, Address, Token, NotFound }`
  plus `pub enum AddressKind { Eoa, Contract }` and light wrappers
  `BlockSummary`, `TxSummary`.

Ports (`src/application/ports/`):

- `block_lookup.rs` — `get_by_hash`, `get_by_number`.
- `tx_lookup.rs` — `get(hash, chain)`.
- `address_lookup.rs` — `classify(addr, chain) -> AddressKind`.
- `ens_resolver.rs` — `forward(name, chain) -> Option<Address>`, `reverse(addr,
  chain) -> Option<String>`.
- `token_search.rs` — `by_symbol(sym, chain) -> Vec<TokenMetadata>` and
  `by_name(text, chain) -> Vec<TokenMetadata>`.

Use case (`src/application/use_cases/resolve_query.rs`):

- Struct `ResolveQuery<'a, B, T, A, E, S>` holding references to the five ports.
- Method `async fn run(input, chain) -> Result<Vec<ResolvedEntity>, DomainError>`.
- Classification follows section 3's table; ambiguous inputs run every
  plausible lookup via `tokio::join!` so the slowest port does not block the
  others.
- Ranking rule: exact-type hits first, ENS resolutions second, plain text
  search last.
- Empty result yields `vec![ResolvedEntity::NotFound { reason }]` with a
  human hint mentioning the chain.

Stubs (`tests/support/stubs.rs`):

- `StubBlockLookupPort`, `StubTxLookupPort`, `StubAddressLookupPort`,
  `StubEnsResolverPort`, `StubTokenSearchPort`. Every stub carries a table
  primed by the test and a `set_broken` switch for the rate-limit scenario.

Tests (`tests/functional/resolve_query.rs`):

- Resolves a tx hash.
- Resolves a block number.
- Resolves an ENS name and exposes the underlying address as a candidate.
- Disambiguates a 64-hex value between tx and block.
- Reports `NotFound` with the active chain name.
- Treats an uppercase 3-char ticker as a token search input.
- Rejects empty input with `DomainError::InvalidInput`.

No UI or HTTP yet. Acceptance: `cargo test --test functional resolve_query`
green, `cargo clippy --all-targets -- -D warnings` clean.

### 10.2 Slice B — Alchemy + Etherscan adapters

Extend `src/adapters/rpc/` with four new adapters sharing the existing
`RpcClient`:

- `AlchemyTxLookup` using `eth_getTransactionByHash`.
- `AlchemyBlockLookup` using `eth_getBlockByHash` and `eth_getBlockByNumber`.
- `AlchemyAddressLookup` using `eth_getCode` for EOA-vs-contract classification.
- `AlchemyEnsResolver` calling the ENS Registry at
  `0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e` via `eth_call`. Forward uses
  `resolver(node)` + `addr(node)`; reverse uses the reverse-resolver dance.

`TokenSearchPort` is now backed by `EtherscanTokenSearch` under
`src/adapters/etherscan/token_search.rs`:

- `by_symbol(ticker, chain)` consults a static ticker table
  (`src/adapters/etherscan/tickers.rs`) indexed by `(Chain,
  ticker_lowercase) -> Address`. The table is seeded with the
  top-of-mind ERC-20s per MVP chain (USDC, USDT, DAI, WETH, WBTC,
  …). On a hit the adapter issues `contract/getsourcecode` to
  enrich the entry with `ContractName`; `decimals` defaults to the
  ERC-20 canonical of 6/18 depending on the curated entry. No
  network call fires when the ticker misses the table.
- `by_name(text, chain)` takes the same table and performs a
  case-insensitive `contains` against the curated names, returning
  every match. No HTTP is issued here.
- When the query looks like an `Address`, the port delegates to
  `contract/getsourcecode` to surface the verified contract name;
  the result is promoted into a `TokenMetadata` only if
  `ContractName` is present (otherwise `Vec::new()`).

The curated ticker file lives under
`src/adapters/etherscan/tickers.rs` as a `const` array — no
secrets, no live market data. Adding / editing tokens is a code
change by design; see `plan/15-backlog.md` §8.3.

When `ETHERSCAN_API_KEY` is missing the composition root keeps
falling back to the `NoopTokenSearch` so the ticker path simply
returns no candidates.

Tests live under `tests/functional/alchemy_{tx,block,address,ens}_lookup.rs`
using wiremock the same way the Home adapters do. New fixtures:

- `rpc__eth_getTransactionByHash__found.json`
- `rpc__eth_getTransactionByHash__not_found.json`
- `rpc__eth_getBlockByHash__found.json`
- `rpc__eth_getBlockByHash__not_found.json`
- `rpc__eth_getCode__eoa.json`
- `rpc__eth_getCode__contract.json`
- `rpc__eth_call__ens_resolver.json`
- `rpc__eth_call__ens_addr.json`

### 10.3 Slice C — UI integration

- `SearchScreen` is pushed on the `ScreenStack` when the user presses `/` from
  `Home`. Enter on a candidate pops the search screen and pushes a minimal
  `DetailPlaceholderScreen` that renders "{entity kind}: {identifier}" until
  the real detail screens land.
- `HomeScreen::handle_key` gains a `/` -> `Command::OpenSearch` branch; a new
  variant is added to [`Command`] so the dispatcher knows to push a
  search screen. The screen creation lives in the dispatcher to keep
  `HomeScreen` free of adapter wiring.
- BDD scenarios from section 6 are wired in `tests/e2e/steps/search.rs`
  through stubs primed in the `World`. The scenarios assert on the title of
  the screen on top of the stack after each navigation.
- `SearchScreen` receives the five ports through generics, mirroring
  `HomeSession`. The composition root in `infra::run` picks stubs for the
  demo path and Alchemy adapters for the live path.

Acceptance for the whole plan: every scenario in
`tests/e2e/features/search.feature` passes, plan status becomes `done` in
`plan/README.md`, `cargo test` + `cargo clippy` remain clean.

## 11. ERC-20 detection and Contract shortcut

A follow-up slice extends the Search results with two additional
rows on address inputs:

- **Contract shortcut** — emitted synchronously whenever the
  address lookup reports `AddressKind::Contract`. Zero additional
  RPC cost: the `eth_getCode` that backs the classify call already
  runs, so the row is just a second entry in the candidate list
  that routes to `ContractDetailScreen` instead of
  `AddressDetailScreen`.
- **Token shortcut** — emitted asynchronously, after a follow-up
  ERC-20 probe. The probe is **pessimistic**: it fires only when we
  already have evidence that the address is a contract. EOAs never
  trigger the probe, non-address inputs (tx hash / block / ENS →
  EOA / ticker / free text) never trigger it either.

### 11.1 Domain

```rust
// src/domain/search.rs
pub enum ResolvedEntity {
    Block { number, hash },
    Tx { hash, block },
    Address { address, kind, ens_name },
    Contract { address },   // NEW — routes to ContractDetailScreen.
    Token(TokenMetadata),
    NotFound { reason },
}
```

### 11.2 ResolveQuery change

`ResolveQuery::run` stays pure and CPU-bound: it appends a
`ResolvedEntity::Contract { address }` row immediately after any
`Address { kind: Contract, .. }` row it pushes (both the
Address-classification path and the EnsName-resolves-to-contract
path). No probe runs here.

### 11.3 Search feed (`src/infra/search_feed.rs`)

The feed gains a sixth port, `TokenReaderPort`, and emits in two
phases:

```mermaid
sequenceDiagram
  participant U as User
  participant S as SearchScreen
  participant F as search_feed task
  participant R as TokenReaderPort

  U->>S: types "0xa0b8..."
  S->>F: input
  F-->>S: update1 (Address, Contract)
  Note over F: base list contains Contract → probe
  F->>R: get(contract, chain)
  R-->>F: Some(overview) | None | Err
  F-->>S: update2 (Address, Contract, Token) if Some
```

The `SearchFeedUpdate` stale-input guard already in place on
`SearchScreen` ensures that replies for stale inputs are dropped,
so the two-phase emission works without extra coordination.

### 11.4 Ordering invariant

The row order is always `[Address, Contract?, Token?]`. The
`Address` row stays first so pressing Enter without arrow keys
reproduces the pre-existing default behaviour (opens
AddressDetail, which itself exposes the Contract and inline Token
tabs when applicable — see `plan/6-address-detail.md` §12.4.3 and
§12.4.4).

### 11.5 Tests

- Functional `tests/functional/resolve_query.rs`:
  - `contract_address_also_emits_contract_shortcut_after_the_address_row`.
  - `eoa_address_does_not_emit_contract_shortcut`.
  - `ens_that_resolves_to_contract_also_emits_contract_shortcut`.
- Functional `tests/functional/search_feed_token_probe.rs`:
  - `eoa_input_never_probes_token_reader` (call counter asserts 0).
  - `contract_without_metadata_probes_once_and_emits_only_base_update`.
  - `erc20_contract_emits_two_updates_with_token_appended`.
- BDD `tests/e2e/features/search.feature`:
  - `Address input for a contract surfaces a Contract shortcut row`.
  - `Address input for an ERC-20 contract also surfaces a Token row after the probe`.

### 11.6 Infra wiring

`build_live_stack` in `src/infra/mod.rs` constructs
`AlchemyTokenReader` once and passes it to `search_feed::spawn`
alongside the other five ports. The BDD step helper in
`tests/e2e/steps/search.rs::build_search_factory` now invokes the
production `search_feed::spawn` directly so the BDD coverage
exercises the same code path as live.

## 12. Input hygiene and resolution cache

Follow-up slice (shipped with the §8.3 backlog work — see
`plan/15-backlog.md`). Folds together three deferred concerns:
URL paste, input normalisation, and a TTL cache for resolved
candidate lists.

### 12.1 `classify_input` — dedicated normalisation helper

`ResolveQuery::run` and `search_feed::spawn` both call a new pure
helper `classify_input(raw: &str) -> ClassifiedInput` declared
next to `classify` in
`src/application/use_cases/resolve_query.rs`. The returned struct
carries the normalised input and a `Classification` value:

```rust
pub struct ClassifiedInput {
    pub normalised: String,
    pub classification: Classification,
}
```

The helper:

1. Strips leading / trailing whitespace.
2. Strips one pair of matching quotes (`"…"`, `'…'` or the
   curly-quote pair `"…"`) from the outside.
3. Unwraps a block-explorer URL when one is pasted (see 12.2).
4. Lowercases the hex body of `0x…` inputs (preserves ticker
   casing).
5. Dispatches to the existing `classify` table for the final
   classification.

The search feed uses `classified.normalised` as the cache key so
`0xDEAD…` and `0xdead…` share a single cache slot.

### 12.2 URL paste support

`classify_input` recognises paths of the shape `/tx/{value}`,
`/address/{value}` or `/block/{value}` under the following hosts:

- `etherscan.io` (+ `www.etherscan.io`, `sepolia.etherscan.io`,
  `optimistic.etherscan.io`).
- `polygonscan.com`, `basescan.org`, `arbiscan.io`.

When the URL matches, the trailing value becomes the effective
input before the classification table runs. Other URLs are treated
as free text (they will fall through to `FreeText` and produce
`NotFound`, which is acceptable).

URL chain hints (`polygonscan.com` → `Chain::Polygon`, etc.) are
**not** consumed by `ResolveQuery` — the active chain stays
whatever the user selected in Settings. The rule is documented
here and revisited if users ask for automatic chain switching.

### 12.3 `TtlCache` adapter

`src/adapters/cache/memory.rs` introduces a generic, thread-safe,
in-memory TTL cache:

```rust
pub struct TtlCache<K, V, C: Clock = SystemClock> { ... }

impl<K, V, C: Clock> TtlCache<K, V, C>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    C: Clock + Send + Sync + 'static,
{
    pub fn with_ttl_and_clock(ttl: Duration, clock: C) -> Self;
    pub fn get(&self, key: &K) -> Option<V>;
    pub fn insert(&self, key: K, value: V);
}
```

Internals: `tokio::sync::RwLock<HashMap<K, (V, Instant)>>`. On
`get`, expired entries are removed opportunistically. Time comes
from the injected `Clock` port (12.5); tests use
`FrozenClock::advance` to step past the TTL.

This adapter is cross-cutting: search uses it with
`Key = (Chain, String)` and `Value = Vec<ResolvedEntity>`; future
consumers (ENS 5 min, Etherscan ABI, Alchemy prices) will re-use
the same type. Resets are a drop: `TtlCache` implements `Clone`
(cheap `Arc` clone) so the composition root can share a single
instance across screens.

### 12.4 Search TTL cache wiring

`search_feed::spawn` now accepts an optional
`Arc<TtlCache<(Chain, String), Vec<ResolvedEntity>>>`:

- On input: look up `(chain, normalised)` in the cache. On hit
  publish a single update with the cached list and skip the
  ERC-20 probe (the probe result, when applicable, was cached at
  the previous run).
- On miss: call `ResolveQuery::run`, publish the base update,
  run the ERC-20 probe, publish the enriched update, then
  `cache.insert((chain, normalised), enriched.clone())`. A
  missing ERC-20 probe result caches the base list, so a repeat
  search within 60 s still avoids the network.

The cache is bypassed when `NotFound` is the only row (so a
transient network hiccup is not sticky for the full TTL).

### 12.5 `Clock` port

`src/application/ports/clock.rs` declares a minimal `Clock` trait
with a single method `now(&self) -> Instant`. Adapters:

- `SystemClock` in `src/adapters/clock.rs` — `Instant::now()`.
- `FrozenClock` in `tests/support/stubs.rs` — holds a
  `Mutex<Instant>`; `advance(Duration)` moves it forward.

This covers the deferred work listed in `plan/15-backlog.md`
§8.12 for Clock + `FrozenClock`.

### 12.6 Tests

- Unit: `classify_input` rstest table under
  `tests/functional/resolve_query.rs` — trimming, surrounding
  quotes, uppercase hex, URL paste per supported host, garbage
  URL falling through to FreeText.
- Functional: `tests/functional/ttl_cache.rs` — hit within TTL,
  expiry after `clock.advance(ttl + 1s)`, concurrent reads do
  not deadlock, insert overwrites the timestamp.
- BDD: `tests/e2e/features/search.feature` gets:
  - `Scenario: Paste an Etherscan tx URL` — the trailing hash
    resolves as a transaction.
  - `Scenario: Paste an Etherscan address URL`.
  - `Scenario: Paste an Etherscan block URL`.
  - `Scenario: Repeated search within 60 s reuses the cache`
    — asserts on a recording block-lookup stub whose
    `call_count` stays at 1 across two searches.
- Fixture(s):
  - `cache__ttl_cache__hit.json`, `cache__ttl_cache__expired.json`
    are narrative fixtures used by the rstest table for timing
    documentation.
  - `etherscan__getsourcecode__usdc.json` (already present) covers
    the `EtherscanTokenSearch` happy path; a new
    `etherscan__token_search__rate_limit.json` captures the 429
    error case.

### 12.7 Out of scope

- Persistent cache on disk: remains out of scope (rules already
  exclude persistent indexing).
- Automatic chain switching from URL hints — parked with a note in
  §12.2 but not shipped.

## 13. Overlay layout (input footer + results modal)

Shipped with `slice2/search-overlay`. Fixes the Bug 1 regression
reported in the parent plan
(`plan/unified_detail_screen_+_global_ux`): pressing `/` used to
render `SearchScreen` across the full `frame.area()`, visually
erasing whatever screen was underneath. The new layout treats the
search screen as a real overlay: the backing screen (Home,
AddressDetail, …) keeps rendering behind and only two small
sub-rectangles are cleared and painted.

### 13.1 Geometry

`SearchScreen::render` receives the full `frame.area()` and
derives two rectangles:

- `input_rect = footer_rect(area, 3)` — a 3-row strip pinned to
  the bottom of the screen: `x = area.x`, `width = area.width`,
  `height = 3`, `y = area.bottom() - 3`. Vim-style `:` command
  line.
- `results_rect = centered_rect(area, 60, 50)` — a centered
  floating block at 60% width × 50% height. Reuses the existing
  helper in `src/adapters/ui/modal.rs` (which is now `pub(crate)`
  so both the Help modal and the Search overlay share a single
  implementation).

### 13.2 Clear boundary

`Clear` is rendered only inside `input_rect` and `results_rect`.
No `Clear` on `area` itself. Every other cell is left as the
backing screen painted it, so Home / AddressDetail / … stays
visible around the overlay.

### 13.3 Overlap guard

On typical terminal sizes (≥ 80×24) `results_rect` ends well
above `input_rect`. The guard exists for degenerate dimensions:
when `results_rect.bottom() > input_rect.top()`, the results
rectangle is shrunk so its bottom edge sits one row above the
input strip. If that leaves zero rows, the results rectangle is
hidden for that frame and the user still sees the input bar. The
input bar is never clipped — the user must always be able to
type.

### 13.4 Wiring contract

The dispatcher in `src/infra/runtime.rs::redraw` keeps its
existing two-phase contract: the stack's top screen renders at
the full area first, and then the modal (including
`SearchScreen` when opened via `Command::OpenModal`) renders at
the same full area. The overlay honours this contract by drawing
into sub-rectangles only — no change is required in the runtime.

### 13.5 Tests

- Functional snapshot
  `tests/functional/search_overlay_render.rs`: a 120×30
  `TestBackend` first renders `HomeScreen`, then the
  `SearchScreen` over the same buffer (mirroring the two-phase
  `redraw` call). Assertions:
  - Home's `"Chain: Ethereum"` header row is still present.
  - Home's Network / Gas Tracker cards are still visible outside
    the overlay rectangles.
  - The input strip at the bottom contains the `> ` search prompt
    (assertions scan only the bottom three rows so they do not
    match `> [kind]` in the Candidates list).
  - The results block shows its `Candidates` border title.
- Functional `tests/functional/search_screen_input.rs`: line editor
  (insert in the middle, backspace before cursor, delete forward).
- BDD `tests/e2e/features/search.feature`: new scenario
  `Opening / overlays search on top of Home without erasing it`.
  Exercises the `OpenModal` path (mirroring the runtime) and
  asserts, via a `TestBackend` composite render, that Home's
  content still shows through.

### 13.6 Input line editing

The footer strip is a real line editor, not append-only typing:

- **Cursor** — byte index on UTF-8 character boundaries; shown as a
  block caret (reversed video on the next character, or a reversed
  trailing space at end-of-input).
- **Blink** — `cursor_blink_on` toggles on each `Screen::tick` (same
  cadence as `AppEvent::Tick`, 250 ms in `runtime.rs`), so the caret
  blinks without extra `Command` variants.
- **Keys** — printable characters insert at the cursor; `Backspace`
  deletes before the cursor; `Delete` deletes at the cursor; `Left` /
  `Right` move the cursor. `Up` / `Down` continue to move only the
  candidate list selection.
