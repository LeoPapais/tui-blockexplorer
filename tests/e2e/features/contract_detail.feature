Feature: Contract detail
  # Mirrors plan/7-contract-detail.md sections 12.3 and 12.4.1.
  # Scenarios drive the ContractDetailScreen through the stub ports
  # held in the cucumber World.

  Background:
    Given the user is on Home
    And the active chain is "ethereum"

  Scenario: Open plain contract without proxy
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    When the user opens ContractDetail for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    Then a "Contract" screen is on top
    And once the contract is loaded, no proxy is detected

  Scenario: Open EIP-1967 proxy contract
    Given the address reader knows contract "0xa0a1000000000000000000000000000000000001" with balance 0 and nonce 1
    And the proxy detector reports EIP-1967 implementation "0xb0b1000000000000000000000000000000000002" for "0xa0a1000000000000000000000000000000000001"
    When the user opens ContractDetail for "0xa0a1000000000000000000000000000000000001"
    Then a "Contract" screen is on top
    And once the contract is loaded, the proxy points at "0xb0b1000000000000000000000000000000000002"

  Scenario: Verified contract shows Source + ABI
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    And the contract source stub has a verified single-file source for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    When the user opens ContractDetail with source for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    Then a "Contract" screen is on top
    And once the contract source is loaded, the verified flag is true
    And the Source tab lists 1 file
    And the ABI tab is populated

  Scenario: Unverified contract shows empty Source tab
    Given the address reader knows contract "0xdead000000000000000000000000000000000001" with balance 0 and nonce 0
    When the user opens ContractDetail with source for "0xdead000000000000000000000000000000000001"
    Then a "Contract" screen is on top
    And once the contract is loaded, the source is unavailable
