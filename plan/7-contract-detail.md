# 7 — Contract Detail

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
