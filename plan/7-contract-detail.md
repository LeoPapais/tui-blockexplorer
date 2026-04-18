# 7 — Contract Detail

Status: **done** — MVP Overview + EIP-1967 proxy detection shipped
earlier; Source, ABI, Read, Events and Storage tabs landed next; and
section 12.5 wraps up the April-2026 follow-ups (UUPS + Transparent
slot probing, Etherscan proxy hint, Events tab pagination with
next/prev window and Solidity keyword highlighting on the Source
tab). The Write tab and the decompiler integration stay deferred;
see section 13.

Inspect and read a smart contract. Tabs in MVP: Overview, Source, ABI, Read, Events,
Storage. The Write tab is deferred because it requires a signer.

## 1. Purpose and user goals

- Confirm whether a contract is verified, and show its source when it is.
- Let developers browse ABI and call read-only functions from the terminal.
- Detect common proxy patterns transparently and let the user toggle between proxy
  and implementation.

## 2. Layout

```
+-- Breadcrumb ----------------------------------------------------+
| ... > Contract 0xa0b8...                                         |
+-- Header --------------------------------------------------------+
| 0xa0b8...  Verified  Proxy (EIP-1967)  -> 0xB0B1...              |
| Compiler 0.8.19   Optimizer on   Runs 200                        |
+-- Tabs ----------------------------------------------------------+
| [Overview] Source ABI Read Events Storage                        |
+-- Content -------------------------------------------------------+
| Deployer  0xCdE...  at block 16,000,000                          |
| Proxy     EIP-1967 transparent                                   |
| Impl      0xB0B1...  (verified)                                  |
| Admin     0xEF12...                                              |
+-- Status bar ----------------------------------------------------+
| Tab tabs  i toggle impl  y copy address                          |
+------------------------------------------------------------------+
```

Tabs:

- **Overview**: deployer, deploy tx, proxy info, compiler metadata.
- **Source**: syntax-highlighted Solidity / Vyper files with a file picker when the
  source has multiple files.
- **ABI**: scrollable JSON ABI with collapsible sections per function / event.
- **Read**: list of `view` and `pure` functions with editable argument fields and a
  result pane per call.
- **Events**: recent event log entries decoded with the contract ABI.
- **Storage**: raw storage slot inspector (requires slot index input).

## 3. Keybindings

| Key     | Action                                                    |
|---------|-----------------------------------------------------------|
| `Tab`   | Next tab                                                  |
| `i`     | Toggle between proxy and implementation (when applicable) |
| `Enter` | On Read tab, execute the selected function call           |
| `y`     | Copy address                                              |
| `/`     | Filter ABI functions by name                              |

## 4. Use cases

### 4.1 `LoadContractOverview`

- **Input**: `Address`, chain.
- **Output**: `ContractOverview { deployer, deploy_tx, verified, compiler,
  optimizer, proxy_kind, impl_address }`.
- **Ports**: `AddressReaderPort::code`, `ContractSourcePort::get_metadata`,
  `ProxyDetectionPort::detect`, `TransfersPort::first_external_tx`.

### 4.2 `LoadContractSource`

- **Input**: `Address`, chain.
- **Output**: `ContractSource { files: Vec<SourceFile>, abi: Abi }`.
- **Ports**: `ContractSourcePort::get_source`.

### 4.3 `InvokeReadFunction`

- **Input**: `Address`, function signature, argument values, optional block tag.
- **Output**: decoded return tuple.
- **Ports**: `ContractReaderPort::call`.
- **Behaviour**: encodes arguments against the ABI, issues `eth_call`, decodes
  return data. Reverts produce a `DomainError::ExecutionReverted { reason }`.

### 4.4 `DetectProxyImplementation`

- **Input**: `Address`, chain.
- **Output**: `Option<ProxyInfo { kind, implementation, source }>`.
- **Ports**: `ProxyDetectionPort::detect`, optionally composed with
  `EtherscanProxyHintPort::implementation_hint` (see 12.5.1).
- **Behaviour**: reads three EIP-1967-family slots via
  `eth_getStorageAt` at tag `"latest"` and returns the first non-
  zero result (see 12.5.2):
  - EIP-1967 impl slot
    `0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc`
    → `Eip1967`, `source = Eip1967Slot`.
  - EIP-1822 (UUPS) PROXIABLE slot
    `0xc5f16f0fcc639fa48a6947836d9850f504798523bf8c9a3a87d5876cf622bcf7`
    → `Uups`, `source = Eip1822Slot`.
  - OpenZeppelin Transparent admin slot
    `0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103`
    → `Transparent`, `source = TransparentSlot`.
  When all three slots are zero and an Etherscan key is available,
  the composite detector asks `getsourcecode`'s `Implementation`
  field. When that returns an address, the result is surfaced as
  `Eip1967` with `source = EtherscanHint`.

### 4.5 `LoadContractEvents`

- **Input**: `Address`, chain, block range, optional topic filter.
- **Output**: `Vec<DecodedLog>`.
- **Ports**: `EventLogPort::get_logs`, `ContractSourcePort::get_abi`,
  `SignatureDirectoryPort`.

## 5. Ports required

- `ContractSourcePort`: `get_metadata`, `get_source`, `get_abi`.
- `ProxyDetectionPort`: `detect(addr, chain)`.
- `ContractReaderPort`: `call(addr, function, args, block)`.
- `EventLogPort`: `get_logs(filter)`.
- `StoragePort`: `get_at(addr, slot, block)`.

## 6. Data sources

- Alchemy: `eth_getCode`, `eth_getStorageAt`, `eth_call`, `eth_getLogs`.
- Etherscan V2: `getsourcecode`, `getabi`.
- openchain.xyz and Samczsun: event topic fallback on the Events tab.

## 7. BDD scenarios (`tests/e2e/features/contract_detail.feature`)

```gherkin
Feature: Contract detail

  Background:
    Given the active chain is "ethereum"

  Scenario: Verified contract shows source
    Given the Etherscan stub returns verified source for "0xa0b8..."
    When the user opens ContractDetail for "0xa0b8..."
    Then the Overview tab says "Verified"
    And the Source tab shows at least one file with syntax-highlighted content

  Scenario: Unverified contract shows bytecode only
    Given the Etherscan stub has no source for "0xdead..."
    When the user opens ContractDetail for "0xdead..."
    Then the Overview tab says "Unverified"
    And the Source tab shows the first 512 bytes of the bytecode

  Scenario: Proxy is detected and can be toggled
    Given EIP-1967 storage returns implementation "0xB0B1..."
    When the user opens ContractDetail for "0xA0A1..."
    Then the header shows "Proxy (EIP-1967) -> 0xB0B1..."
    When the user presses "i"
    Then the screen rerenders with "0xB0B1..." as the current address

  Scenario: Read function returns decoded value
    Given the ABI defines "balanceOf(address) returns (uint256)"
    And "eth_call" returns 1000000 for "0xd8dA..."
    When the user selects "balanceOf" on the Read tab
    And enters "0xd8dA..." and presses Enter
    Then the result pane shows "1000000"

  Scenario: Read function reverts gracefully
    Given "eth_call" reverts with reason "InsufficientBalance()"
    When the user executes the call
    Then the result pane shows an error row with the reason string
```

## 8. Functional tests

- `LoadContractOverview`: verified; unverified; proxy detected; no proxy.
- `LoadContractSource`: single-file source; multi-file source; Vyper source.
- `InvokeReadFunction`: success; revert; argument type mismatch returns
  `DomainError::InvalidArgument`.
- `DetectProxyImplementation`: EIP-1967 transparent; UUPS via fallback call;
  non-proxy returns `None`.
- `LoadContractEvents`: ABI-decoded; topic fallback via openchain.

## 9. Fixtures

- `rpc__eth_getCode__contract_a0b8.json`
- `rpc__eth_getStorageAt__eip1967_impl.json`
- `rpc__eth_getStorageAt__eip1967_zero.json`
- `rpc__eth_call__balanceOf_success.json`
- `rpc__eth_call__revert_insufficient_balance.json`
- `rpc__eth_getLogs__usdc_transfer_range.json`
- `etherscan__getsourcecode__verified_single_file.json`
- `etherscan__getsourcecode__verified_multi_file.json`
- `etherscan__getsourcecode__not_verified.json`
- `openchain__event__transfer_topic0.json`

## 10. Resolved questions

All three open questions below are **resolved** and shipped under
section 12.5 (April 2026, branch
`probe/8.8-contract-detail-followups`):

- Events tab pagination — **block range, default 5,000 blocks**, with
  `N` / `Shift+N` actions to page backwards / forwards through
  windows. Plan 12.5.3. The previous single-window UI is now the
  `window 0` case of the pagination UI.
- Etherscan proxy hint as a second source when EIP-1967 detection
  returns zero — **yes**, use the `Implementation` field from
  Etherscan `contract/getsourcecode` as a composite fallback. Plan
  12.5.1.
- Syntax highlighting for the Source tab — **hand-rolled minimal
  Solidity highlighter** shipped today; `syntect` stays documented
  as the preferred future route once we decide whether to bundle a
  Solidity `.sublime-syntax` or swap to `tree-sitter-solidity`.
  The minimal highlighter covers keywords, primitive types, numeric
  and string literals, and `//` / `/* */` comments; `.vy` (Vyper)
  stays plain text until we pick a parser. Plan 12.5.4.

## 11. Deferred

- Write tab with transaction signing (requires a signer — hardware
  wallet, browser extension or plaintext key; parked on the
  credential-strategy decision).
- Decompiler integration (panoramix, heimdall) for unverified
  contracts.
- Upgrading the Solidity highlighter to a full grammar via
  `syntect` (bundled `.sublime-syntax`) or `tree-sitter-solidity`.
  See 12.5.4.
- Vyper (`.vy`) syntax highlighting — currently renders as plain
  text next to the Solidity highlighter; a shared tokeniser trait
  lands together with the highlighter upgrade above.

## 12. Implementation plan

Three slices. MVP lands the Overview tab with a lean header plus
EIP-1967 proxy detection; Source / ABI / Read / Events / Storage
tabs all wait on adapters we have not written yet (Etherscan and a
contract-reader built on `eth_call`). Search already routes
`ResolvedEntity::Address` with `kind = Contract` to a separate
screen after this plan lands.

### 12.1 Slice A — domain + port + use case + stubs

Domain (`src/domain/`):

- `contract.rs`
  - `pub enum ProxyKind { Eip1967 }` — only one variant for MVP; UUPS
    and transparent detection join later.
  - `pub struct ProxyInfo { kind: ProxyKind, implementation: Address }`.
  - `pub struct ContractOverview { account: AddressOverview, proxy:
    Option<ProxyInfo> }` — reuses `AddressOverview` so balance /
    nonce / kind rendering stays consistent with plan 6.

Port (`src/application/ports/proxy_detection.rs`):

```rust
pub trait ProxyDetectionPort: Send + Sync {
    async fn detect(&self, address: Address, chain: Chain)
        -> Result<Option<ProxyInfo>, DomainError>;
}
```

Use case `load_contract_overview` composes `AddressReaderPort::get`
and `ProxyDetectionPort::detect`, returning a `ContractOverview` or
`DomainError::NotFound` when the address itself does not resolve.

Stub `StubProxyDetectionPort` with `set(address, info)` helper.

Functional tests `tests/functional/load_contract_overview.rs`:
- contract without proxy (plain kind = Contract, proxy = None),
- contract with EIP-1967 proxy wired to an implementation,
- missing address surfaces NotFound.

### 12.2 Slice B — Alchemy adapter

`src/adapters/rpc/proxy_detection.rs` exposes
`AlchemyProxyDetector` that reads the EIP-1967 implementation slot
`0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc`
via `eth_getStorageAt` at tag `"latest"`, decodes the last 20 bytes
and returns:
- `None` when the slot is all zeros,
- `Some(ProxyInfo { kind: Eip1967, implementation })` otherwise.

Tests `tests/functional/alchemy_proxy_detector.rs` cover a non-proxy
address (zero slot) and a proxy address (populated slot). Fixtures:
- `rpc__eth_getStorageAt__eip1967_zero.json` (already exists from
  plan/2 search era) — reused.
- `rpc__eth_getStorageAt__eip1967_impl.json` — new.

### 12.3 Slice C — UI + wiring + BDD

- `src/adapters/ui/contract_detail.rs` — `ContractDetailScreen` with
  a single Overview tab rendering:
  - account line (address, kind, balance, nonce),
  - proxy badge when detected, including the implementation address,
  - a "deferred tabs" footer note listing what is not yet wired
    (Source / ABI / Read / Events / Storage).
- `src/infra/contract_feed.rs` + `spawn` helper (channel pair +
  ContractOverview publisher backed by the two ports).
- Search detail factory routes
  `ResolvedEntity::Address { kind: Contract, .. }` to the new
  `ContractDetailScreen`; EOAs keep going to `AddressDetailScreen`.
- BDD `tests/e2e/features/contract_detail.feature`:
  - Open a plain contract: kind "Contract", no proxy badge.
  - Open an EIP-1967 proxy: proxy badge shows the stubbed
    implementation address.

Acceptance: plan flips to `done (MVP)`, plan/README updated,
cargo clippy clean, all tests green.

## 12.4 Expanded slices (delivered in this iteration)

Three commits add the remaining tabs on top of the MVP Overview.

### 12.4.1 Commit 1 — Source + ABI tabs

Domain extension (`src/domain/contract_source.rs`):

- `ContractSource { is_verified, contract_name, compiler_version,
  optimizer_enabled, optimizer_runs, evm_version, license, abi,
  files: Vec<SourceFile>, implementation: Option<Address> }`.
- `SourceFile { path, content }`.
- A standalone `parse_etherscan_source_envelope` helper that
  decodes Etherscan's quirky triple-shape `SourceCode` field: a
  raw single-file string, a single-level JSON object keyed by
  file path, or the `{{ ... }}` double-wrapped JSON used for
  multi-file contracts.

Port extension: `ContractSourcePort` gains
`get_source(addr, chain) -> Option<ContractSource>` alongside the
existing `get_abi`. `EtherscanContractSource` adapter implements
both; unverified contracts surface as `Ok(None)`.

UI: `ContractDetailScreen` grows a tab bar rendered through
`ratatui::widgets::Tabs` (stable highlight; mirrors the TxDetail
and AddressDetail styling). Tabs introduced:

- **Source**: file picker on the left (`Up`/`Down` to switch file
  when multiple are present) + content pane on the right with
  per-file vertical scroll. Falls back to a short hex dump of the
  bytecode when the source is unverified.
- **ABI**: pretty-printed ABI JSON with vertical scroll.

A new `ContractSourceFeed` channel delivers the source payload as
soon as it arrives, so Overview and proxy detection stay snappy.

### 12.4.2 Commit 2 — Read tab

Domain (`src/domain/contract_read.rs`):

- `AbiParamType` enum covering `Uint { bits }`, `Int { bits }`,
  `Address`, `Bool`, `String`, `Bytes`, `BytesN(usize)`. Other
  variants (arrays, tuples, mappings) fall through as
  `AbiParamType::Unsupported(raw: String)` so the UI can still
  list the function with a clear reason.
- `AbiFunction { name, signature, inputs: Vec<AbiParam>, outputs }`,
  `AbiParam { name, kind }`.
- `AbiValue` (inputs) and `DecodedValue` (outputs) enums with a
  matching shape, plus `AbiValue::from_string(kind, raw)` helpers.

Port `ContractReaderPort::call(address, function, args, chain)
-> Vec<DecodedValue>`, backed by `eth_call` at `"latest"`.

Adapter (`src/adapters/rpc/contract_reader.rs`): a handwritten
minimal ABI codec. Encoding supports the head-section static types
listed above plus `string` and `bytes` (dynamic types are written
after the head as `offset -> length -> padded data`). Decoding
follows the same grammar. Revert reasons are extracted from the
standard `Error(string)` revert selector (`0x08c379a0`) and
surface as `DomainError::ExecutionReverted { reason }`.

UI: Read tab shows a two-pane layout. Left list enumerates every
`view` / `pure` function parsed from the ABI; right pane edits
arguments as free-text and displays the last result. Functions
whose inputs include `Unsupported` types render but refuse
execution with a one-line message.

BDD:
- Read tab executes `balanceOf(address)` and renders the decoded
  u256.
- Read tab surfaces the revert reason when `eth_call` reverts.

### 12.4.3 Commit 3 — Events + Storage tabs

Ports:

- `EventLogPort::get_logs(address, chain, range, topic0?) ->
  Vec<LogEntry>` — paginated by block range with a default window
  of 5,000 blocks.
- `StoragePort::get_at(address, chain, slot) -> [u8; 32]`.

Alchemy adapters wrap `eth_getLogs` and `eth_getStorageAt`.

UI:

- **Events**: lists the latest N logs for the current contract;
  topic0 decoding goes through the signature directory when the
  ABI does not match. `PageUp` / `PageDown` walks the block
  range.
- **Storage**: a simple slot input (`0..`, hex or decimal) +
  a read-out panel with the value in hex, decimal and — when the
  first 12 bytes are zero — address format.

BDD:
- Events tab renders decoded log rows.
- Storage tab reads the requested slot.

## 12.5 April 2026 follow-ups (branch `probe/8.8-contract-detail-followups`)

Four loose ends from sections 10 / 11 / 13 landed together in this
slice. The two WONT-DO items (Write tab, Decompiler integration)
stay in section 13.

### 12.5.1 Etherscan proxy hint as a second source

**Problem.** Some upgradeable patterns (Minimal Proxy / EIP-1167,
diamonds, older custom proxies) leave the EIP-1967 implementation
slot zeroed even though Etherscan knows the implementation through
its own proxy flag. Today `AlchemyProxyDetector::detect` returns
`None` in that case and downstream ABI resolution falls straight
through to "unverified".

**Fix.**

- New application port
  `EtherscanProxyHintPort::implementation_hint(address, chain) ->
  Option<Address>` backed by the existing `getsourcecode` adapter —
  the `Implementation` field is already parsed inside
  `EtherscanContractSource::get_source`, so the new adapter
  (`src/adapters/etherscan/proxy_hint.rs`) only extracts the
  address without any new HTTP shape.
- New adapter `CompositeProxyDetector` in
  `src/adapters/rpc/composite_proxy_detection.rs` composes any
  `ProxyDetectionPort` (primary) with an
  `EtherscanProxyHintPort` (fallback). When the primary returns
  `None` and the fallback returns `Some`, the composite emits
  `ProxyInfo { kind: ProxyKind::Eip1967, implementation,
  source: ProxySource::EtherscanHint }`.
- Domain type `ProxySource { Eip1967Slot, Eip1822Slot,
  TransparentSlot, EtherscanHint }` is added to `ProxyInfo` so the
  UI can distinguish hint-based detection from slot-based
  detection. The Overview tab renders the source inline after the
  implementation address.
- Results from the hint adapter are cached behind the existing
  `TtlCache<Address, Address>` with a 5-minute TTL, matching
  §8.3's cache TTL rule-of-thumb.

**Wiring.** `live_contract_detail_screen` (and the tx-detail pipeline
where it follows proxy ABIs) wires the composite detector when an
Etherscan key is available. When the key is absent the composite
degrades to the base detector and the hint fallback is a no-op.

**Tests.** Functional test for the hint adapter
(`tests/functional/etherscan_proxy_hint.rs`, two `wiremock`
scenarios: proxy + non-proxy row) and for the composite
(`tests/functional/composite_proxy_detector.rs`, four cases:
primary hits / primary misses and fallback hits / both miss /
primary error short-circuits).

### 12.5.2 UUPS and Transparent proxy detection

**Problem.** The existing detector only probes the EIP-1967
implementation slot. UUPS contracts that follow EIP-1822 keep a
different slot populated, and Transparent proxies keep the admin
slot populated alongside the impl slot.

**Fix.** `AlchemyProxyDetector::detect` now probes three slots in
order and returns the first non-zero result:

1. EIP-1967 impl slot
   `0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc`
   → `ProxyKind::Eip1967`, `ProxySource::Eip1967Slot`.
2. EIP-1822 (UUPS) PROXIABLE slot
   `0xc5f16f0fcc639fa48a6947836d9850f504798523bf8c9a3a87d5876cf622bcf7`
   → `ProxyKind::Uups`, `ProxySource::Eip1822Slot`.
3. OpenZeppelin Transparent admin slot
   `0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103`
   → `ProxyKind::Transparent`, `ProxySource::TransparentSlot`.

The slot returned in case 3 is technically the admin address, not
the implementation. For MVP we surface it under the existing
`implementation` field so the Overview tab stays stable; the UI
labels the row as "admin" when `kind == Transparent` so the user
knows what the address means. A future follow-up (tracked in §13)
will read both impl + admin slots and render them as separate
rows.

**Tests.** Functional tests in
`tests/functional/alchemy_proxy_detector.rs` extend the existing
two cases to five: zero slot, EIP-1967 hit, UUPS hit, Transparent
admin hit, all three zero. New fixtures under `tests/fixtures/`
cover each slot response.

### 12.5.3 Events tab pagination by block range

**Problem.** The Events tab calls `eth_getLogs` once at open time
with the `u64::MAX` sentinel ("latest"). Users cannot browse older
events without leaving the screen.

**Fix.**

- Domain type `EventsPage { logs, window: BlockRange, has_older }`
  in `src/domain/events.rs`.
- Use case
  `load_contract_events_page(network_status, event_log, address,
  chain, head: Option<BlockNumber>, offset: u32) -> EventsPage`
  in `src/application/use_cases/load_contract_events_page.rs`.
  `head` defaults to the current chain head (via
  `NetworkStatusPort::snapshot`); `offset` picks which 5_000-block
  window to read (0 = newest). `has_older` is `true` when `window.from > 0`.
- UI state in `ContractDetailScreen`:
  - Initial request uses `offset = 0`.
  - `n` (KeyCode `Char('n')`) increments `offset`, fetches the next
    older window.
  - `Shift+N` (`Char('N')`) decrements `offset`, fetches the newer
    window; floors at 0.
  - Header renders `window N..M  (page k)` where `N..M` are the
    resolved block numbers and `k` is `offset + 1`.
  - When the page is empty, the body renders `No events in this
    window — press [n] for older blocks.`.
- `EventsRequest` now carries `{ head_hint: Option<BlockNumber>,
  offset: u32 }`; `EventsResult` carries `EventsPage` so the UI
  knows the exact range it drew.
- Background task updated to resolve the head via
  `NetworkStatusPort` before the first `get_logs` call and cache it
  across pagination requests; subsequent page fetches reuse the
  cached head to keep the windows aligned.

**Tests.**

- `tests/functional/load_contract_events_page.rs`: happy path
  (offset 0), happy path (offset 3 → window `head - 20_000 + 1..=
  head - 15_000`), edge case (`from` underflows to 0 and
  `has_older` is `false`), failure path (network-status error
  bubbles out).
- `tests/e2e/features/contract_detail.feature` gets two new
  scenarios: "paginates the Events tab backwards" (press `n`,
  assert the visible range moved backwards by 5_000 blocks) and
  "page can return to the newest window with Shift+N".

### 12.5.4 Source tab syntax highlighting (Solidity)

**Decision.** MVP ships a **hand-rolled minimal Solidity
highlighter** under `src/adapters/ui/highlight.rs`. `syntect` is the
preferred long-term route but was **rejected for this slice** because
shipping it without a Solidity `.sublime-syntax` bundled in the
repo buys us nothing over the hand-rolled scanner, and bundling a
real `.sublime-syntax` is out of scope for this worktree (licence
review + asset vendoring). The §13 deferred list keeps syntect and
tree-sitter-solidity queued as upgrade paths.

**Highlighter shape.** A pure function
`highlight_solidity(source: &str, theme: &Theme) -> Vec<Line<'static>>`
returns a `Vec<ratatui::text::Line>` whose spans are coloured through
semantic theme tokens (`keyword`, `type`, `string`, `number`,
`comment`, `plain`). The tokeniser is a small hand-written state
machine that understands:

- `//` line comments and `/* */` block comments (nested `/*` not
  supported — matches Solidity),
- Double-quoted string and hex/byte literals,
- Numeric literals (including `_` separators and `ether` / `wei`
  suffixes),
- Solidity keywords (`pragma`, `contract`, `function`, `returns`,
  `view`, `pure`, `payable`, `public`, `private`, `internal`,
  `external`, `if`, `else`, `for`, `while`, `return`, `new`,
  `delete`, `using`, `library`, `interface`, `struct`, `enum`,
  `event`, `emit`, `modifier`, `constructor`, `try`, `catch`,
  `override`, `virtual`, `abstract`, `import`, `mapping`, `memory`,
  `storage`, `calldata`),
- Primitive types (`uint`, `int`, `uint8`…`uint256` by regex,
  `address`, `bool`, `string`, `bytes`, `bytes1`…`bytes32` by
  regex).

Everything else is emitted as `plain`. The highlighter never
allocates per-character: it produces one span per contiguous token
run.

**UI integration.** `render_source_tab` switches on the selected
file's extension: `.sol` routes through `highlight_solidity`,
everything else (today `.vy` and `.txt`) stays as the existing
plain-`Text` rendering. The `Paragraph` block renders
`Text::from(Vec<Line>)` instead of `Text::from(String)`; wrapping
and scrolling semantics stay identical.

**Tests.**

- `tests/functional/highlight_solidity.rs`: span-level assertions
  (keyword is highlighted with the keyword colour, comment runs
  swallow everything up to the newline, string literal survives
  internal whitespace, numeric literal matches).
- `tests/e2e/features/contract_detail.feature`: "Source tab
  highlights Solidity keywords" scenario asserts that the rendered
  `TestBackend` frame has the `pragma` token styled with the
  keyword theme colour.

## 13. Still deferred (post plan-7 expansion)

- **Write tab**: requires a signer (hardware wallet, browser
  extension or plaintext key) and is out of scope until we pick a
  credential strategy.
- **Decompiler integration** (panoramix / heimdall / etc.) for
  unverified contracts.
- Upgrading the Solidity highlighter to a full grammar via
  `syntect` (bundled `.sublime-syntax`) or
  `tree-sitter-solidity`, plus Vyper highlighting. See 12.5.4.
- Storage slot mapping helpers (e.g. resolving `mapping(address
  => uint)` slot layouts automatically).
- Transparent-proxy: render `implementation` and `admin` as two
  separate rows once the detector reads both slots.
- Historical event streaming beyond the windowed pagination
  (long-lived subscription / `logs` WebSocket).
