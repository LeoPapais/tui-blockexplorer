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

  Scenario: Address input for a contract surfaces a Contract shortcut row
    Given the address stub classifies "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" as a contract
    When the user opens search with "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    Then the primary candidate is "address"
    And the candidates include a "contract" entry for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"

  Scenario: A 7702-delegated EOA routes to AddressDetail
    Given the active chain is "polygon"
    And the address "0x5abc0e99dfc7ba2c9da42f8dc91ec4128a89e919" has code "0xef0100c0ffee…" on that chain
    When the user searches for "0x5abc0e99dfc7ba2c9da42f8dc91ec4128a89e919"
    Then the first result is an Address row flagged as delegated
    And the second result is not a Contract row

  Scenario: Address input for an ERC-20 contract also surfaces a Token row after the probe
    Given the address stub classifies "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" as a contract
    And the token reader knows "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" as "USDC" / "USD Coin" decimals 6 supply 0
    When the user opens search with "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    Then once the ERC-20 probe completes, a "token" candidate "USDC" is appended
    And the primary candidate is still "address"

  Scenario: Paste an Etherscan tx URL
    Given the stub knows the transaction "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b"
    When the user opens search with "https://etherscan.io/tx/0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b"
    Then the primary candidate is "transaction"
    And pressing Enter pushes the "Transaction" screen

  Scenario: Paste an Etherscan address URL
    Given the address stub classifies "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" as a contract
    When the user opens search with "https://etherscan.io/address/0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    Then the primary candidate is "address"
    And the candidates include a "contract" entry for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"

  Scenario: Paste an Etherscan block URL
    Given the stub knows block 21000000
    When the user opens search with "https://etherscan.io/block/21000000"
    Then the primary candidate is "block"
    And pressing Enter pushes the "Block" screen

  Scenario: Uppercase-hex input still resolves to a transaction
    Given the stub knows the transaction "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b"
    When the user opens search with "0x88DF016429689C079F3B2F6AD39FA052532C56795B733DA78A91EBE6A713944B"
    Then the primary candidate is "transaction"
