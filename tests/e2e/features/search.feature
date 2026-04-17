Feature: Universal search
  # Mirrors plan/2-search.md section 6. Each scenario drives the real
  # HomeScreen + SearchScreen + DetailPlaceholderScreen through stubbed
  # ports loaded by the cucumber World.

  Background:
    Given the user is on Home
    And the active chain is "ethereum"

  Scenario: Resolves a valid transaction hash
    Given the stub knows the transaction "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b"
    When the user opens search with "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b"
    Then the primary candidate is "transaction"
    And pressing Enter pushes the "Transaction" screen

  Scenario: Disambiguates between tx hash and block hash
    Given the hash "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b" matches both a transaction and a block in the stub
    When the user opens search with "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b"
    Then there are at least two candidates in the list
    And the first candidate is "transaction"

  Scenario: Resolves an ENS name
    Given "vitalik.eth" resolves to "0xd8da6bf26964af9d7eed9e03e53415d37aa96045" in the stub
    When the user opens search with "vitalik.eth"
    Then the primary candidate is "address"
    And the candidate row shows the ENS name "vitalik.eth"

  Scenario: Resolves a block number
    Given the stub knows block 21000000
    When the user opens search with "21000000"
    Then the primary candidate is "block"
    And pressing Enter pushes the "Block" screen

  Scenario: Not found with actionable hint
    When the user opens search with "0x0000000000000000000000000000000000000000000000000000000000000001"
    Then the candidates list contains a single "not found" entry
    And the hint mentions the active chain name
