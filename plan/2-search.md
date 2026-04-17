# 2 — Universal Search

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

- Should we cache resolution results across opens of the modal? Yes, with a 60s TTL
  per (chain, input). Implemented in `CacheAdapter`.
- Should we support pasting a full URL (for example an etherscan.io link)? Out of
  scope for MVP, easy to add as another classification rule later.
