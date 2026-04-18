# 7 — Contract Detail

Status: **expanded** — MVP Overview tab with EIP-1967 proxy
detection shipped earlier; this phase adds the Source, ABI, Read,
Events and Storage tabs so the screen matches the plan layout.
The Write tab and decompiler integration stay deferred; see
section 13.

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
- **Output**: `Option<ProxyInfo { kind, implementation, admin, beacon }>`.
- **Ports**: `ProxyDetectionPort::detect`.
- **Behaviour**: reads EIP-1967 slots via `eth_getStorageAt`:
  - implementation slot `0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc`
  - admin slot          `0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103`
  - beacon slot         `0xa3f0ad74e5423aebfd80d3ef4346578335a9a72aeaee59ff6cb3582b35133d50`
  If all zero, tries common transparent and UUPS signatures via `eth_call`.

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

## 10. Open questions

- Do we paginate the Events tab by block range or by item count? Decision: block
  range with a configurable window (default 5,000 blocks) plus a next-window action.
- Should we respect Etherscan proxy hints when EIP-1967 detection returns zero?
  Yes, use Etherscan's `Implementation` field from `getsourcecode` as a second
  source of truth.

## 11. Deferred

- Write tab with transaction signing.
- Decompiler integration (panoramix, heimdall) for unverified contracts.

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

## 13. Still deferred (post plan-7 expansion)

- **Write tab**: requires a signer (hardware wallet, browser
  extension or plaintext key) and is out of scope until we pick a
  credential strategy.
- **Decompiler integration** (panoramix / heimdall / etc.) for
  unverified contracts.
- Source file syntax highlighting (the current Source tab renders
  plain text).
- Storage slot mapping helpers (e.g. resolving `mapping(address
  => uint)` slot layouts automatically).
- Historical event streaming with "load older" pagination beyond
  the current page.
