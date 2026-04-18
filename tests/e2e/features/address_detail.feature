Feature: Address detail
  # Mirrors plan/6-address-detail.md sections 12.3 and 12.4.1.
  # Scenarios drive the real AddressDetailScreen through the stub
  # ports held in the cucumber World.

  Background:
    Given the user is on Home
    And the active chain is "ethereum"

  Scenario: Open EOA address from Search
    Given the address reader knows EOA "0xd8da6bf26964af9d7eed9e03e53415d37aa96045" with balance 523140000000000000000 and nonce 1243
    When the user opens AddressDetail for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    Then an "Address" screen is on top
    And once the address is loaded, the Overview shows kind "EOA"
    And the Overview shows balance 523140000000000000000

  Scenario: Open contract address from Search
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    When the user opens AddressDetail for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    Then an "Address" screen is on top
    And once the address is loaded, the Overview shows kind "Contract"

  Scenario: Transactions tab lists transfers and opens TxDetail
    Given the address reader knows EOA "0xd8da6bf26964af9d7eed9e03e53415d37aa96045" with balance 100 and nonce 5
    And the transfers feed knows 2 events for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    When the user opens AddressDetail with transfers for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    And the user switches to the Transactions tab
    Then once loaded, the Transactions tab lists 2 transfers
    When the user selects the first transfer and presses Enter
    Then a "Transaction" screen is on top
