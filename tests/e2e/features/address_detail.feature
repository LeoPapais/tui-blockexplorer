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

  Scenario: Tokens tab lists holdings and opens AddressDetail focused on Token
    Given the address reader knows EOA "0xd8da6bf26964af9d7eed9e03e53415d37aa96045" with balance 100 and nonce 5
    And the portfolio feed knows 2 holdings for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    When the user opens AddressDetail with full feeds for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    And the user switches to the Tokens tab
    Then once loaded, the Tokens tab lists 2 holdings
    When the user selects the first holding and presses Enter
    Then an "Address" screen is on top

  Scenario: Tokens tab empty state
    Given the address reader knows EOA "0xd8da6bf26964af9d7eed9e03e53415d37aa96045" with balance 100 and nonce 5
    And the portfolio feed knows 0 holdings for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    When the user opens AddressDetail with full feeds for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    And the user switches to the Tokens tab
    Then once loaded, the Tokens tab reports no holdings

  Scenario: Contract tab appears for contract addresses
    Given the address reader knows contract "0x1d88182ff972b826f7663591c6270271644171a2" with balance 0 and nonce 1
    And the portfolio feed knows 0 holdings for "0x1d88182ff972b826f7663591c6270271644171a2"
    When the user opens AddressDetail with full feeds for "0x1d88182ff972b826f7663591c6270271644171a2"
    Then once loaded, the tab bar includes the Contract tab

  Scenario: Contract tab is absent for EOAs
    Given the address reader knows EOA "0xd8da6bf26964af9d7eed9e03e53415d37aa96045" with balance 0 and nonce 0
    And the portfolio feed knows 0 holdings for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    When the user opens AddressDetail with full feeds for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    Then once loaded, the tab bar does not include the Contract tab

  Scenario: AddressDetail exposes an inline Token tab for ERC-20 contracts
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    And the token reader knows "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" as "USDC" / "USD Coin" decimals 6 supply 35000000000000
    And the prices stub returns 1.0001 USD for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    And the prices stub returns 24 points for window "1d" on "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    When the user opens AddressDetail with ERC-20 probe for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    Then once the ERC-20 probe completes, the tab bar includes the Token tab
    And the inline Token overview shows symbol "USDC" and price "$1.0001"

  Scenario: AddressDetail does not expose a Token tab for non-ERC20 contracts
    Given the address reader knows contract "0x1d88182ff972b826f7663591c6270271644171a2" with balance 0 and nonce 1
    When the user opens AddressDetail with ERC-20 probe for "0x1d88182ff972b826f7663591c6270271644171a2"
    Then once loaded, the tab bar does not include the Token tab

  Scenario: AddressDetail does not expose a Token tab for EOAs
    Given the address reader knows EOA "0xd8da6bf26964af9d7eed9e03e53415d37aa96045" with balance 100 and nonce 5
    When the user opens AddressDetail with ERC-20 probe for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    Then once loaded, the tab bar does not include the Token tab

  Scenario: Y copies the ENS name when the address has one
    # See plan/6-address-detail.md §11 "Shipped" (`Y` copies ENS,
    # falls back to hex) and plan/15-backlog.md §8.7.
    Given the address reader knows EOA "0xd8da6bf26964af9d7eed9e03e53415d37aa96045" with balance 1 and nonce 1
    And the ENS resolver knows that "0xd8da6bf26964af9d7eed9e03e53415d37aa96045" resolves reverse to "vitalik.eth"
    When the user opens AddressDetail with reverse ENS for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    Then once the overview is loaded, pressing Y copies "vitalik.eth"

  Scenario: Y falls back to the hex address when no reverse ENS is known
    Given the address reader knows EOA "0xd8da6bf26964af9d7eed9e03e53415d37aa96045" with balance 1 and nonce 1
    When the user opens AddressDetail with reverse ENS for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    Then once the overview is loaded, pressing Y copies "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"

  Scenario: CSV export on Portfolio tab copies a CSV
    # plan/6-address-detail.md §11 "Shipped", plan/15-backlog.md §8.7.
    Given the address reader knows EOA "0xd8da6bf26964af9d7eed9e03e53415d37aa96045" with balance 1 and nonce 1
    And the portfolio feed knows 2 holdings for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    When the user opens AddressDetail with full feeds for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    And the user switches to the Tokens tab
    Then once loaded, the Tokens tab lists 2 holdings
    When the user presses e on the Tokens tab
    Then the clipboard sink holds a Tokens CSV with 2 data rows

  Scenario: Tokens tab shows USD totals and a distribution chart
    # plan/6-address-detail.md §11 "Shipped", plan/15-backlog.md §8.7.
    Given the address reader knows EOA "0xd8da6bf26964af9d7eed9e03e53415d37aa96045" with balance 1 and nonce 1
    And the portfolio feed knows priced holdings for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    When the user opens AddressDetail with full feeds for "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    And the user switches to the Tokens tab
    Then once loaded, the Tokens tab shows a USD total of "$2.00" and 1 token not priced
    And the Tokens tab renders at least 2 distribution chart rows

  # ---------------------------------------------------------------------------
  # Contract sub-tabs (migrated from the deleted contract_detail.feature; see
  # plan/16-unified-address-detail.md §8.1).
  # ---------------------------------------------------------------------------

  Scenario: Contract sub-tab opens the Overview by default with no proxy
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    When the user opens AddressDetail as contract for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    Then an "Address" screen is on top
    And once the contract is loaded, no proxy is detected

  Scenario: Contract sub-tab surfaces an EIP-1967 proxy
    Given the address reader knows contract "0xa0a1000000000000000000000000000000000001" with balance 0 and nonce 1
    And the proxy detector reports EIP-1967 implementation "0xb0b1000000000000000000000000000000000002" for "0xa0a1000000000000000000000000000000000001"
    When the user opens AddressDetail as contract for "0xa0a1000000000000000000000000000000000001"
    Then an "Address" screen is on top
    And once the contract is loaded, the proxy points at "0xb0b1000000000000000000000000000000000002"

  Scenario: Source sub-tab shows verified single file
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    And the contract source stub has a verified single-file source for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    When the user opens AddressDetail as contract with source for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    Then once the contract source is loaded, the verified flag is true
    And the Source sub-tab lists 1 file

  Scenario: Unverified contract surfaces the empty Source state
    Given the address reader knows contract "0xdead000000000000000000000000000000000001" with balance 0 and nonce 0
    When the user opens AddressDetail as contract with source for "0xdead000000000000000000000000000000000001"
    Then once the contract is loaded, the contract source is unavailable

  Scenario: Read sub-tab executes a view function and decodes the result
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    And the contract source stub has a verified single-file source for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    And the contract reader stub returns uint 1000000 for "value()" on "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    When the user opens AddressDetail as contract with Read wiring for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    And the user switches to the Read sub-tab
    And the user selects the first function and executes it
    Then once executed, the Read sub-tab shows the uint result 1000000

  Scenario: Read sub-tab surfaces revert reason
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    And the contract source stub has a verified single-file source for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    And the contract reader stub reverts with "InsufficientBalance()" for "value()" on "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    When the user opens AddressDetail as contract with Read wiring for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    And the user switches to the Read sub-tab
    And the user selects the first function and executes it
    Then once executed, the Read sub-tab reports a revert with "InsufficientBalance()"

  Scenario: Events sub-tab lists decoded logs
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    And the event log stub has 2 Transfer events for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    When the user opens AddressDetail as contract with all wiring for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    And the user switches to the Events sub-tab
    Then once loaded, the Events sub-tab lists 2 events

  Scenario: Events sub-tab paginates backwards with "n"
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    And the event log stub has 2 Transfer events for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    When the user opens AddressDetail as contract with all wiring for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    And the user switches to the Events sub-tab
    And the user presses "n" on the Events sub-tab
    Then once reloaded, the Events sub-tab window moved backwards by 5000 blocks

  Scenario: Events sub-tab returns to the newest window with "N"
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    And the event log stub has 2 Transfer events for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    When the user opens AddressDetail as contract with all wiring for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    And the user switches to the Events sub-tab
    And the user presses "n" on the Events sub-tab
    And the user presses "N" on the Events sub-tab
    Then once reloaded, the Events sub-tab page offset is 0

  Scenario: Storage sub-tab reads the requested slot
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    And the storage stub returns the u128 value 42 at slot 0 for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    When the user opens AddressDetail as contract with all wiring for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    And the user switches to the Storage sub-tab
    And the user presses Enter on the Storage sub-tab
    Then once loaded, the Storage sub-tab shows the value 42

  # ---------------------------------------------------------------------------
  # Token sub-tabs (migrated from the deleted token_detail.feature; see
  # plan/16-unified-address-detail.md §8.1).
  # ---------------------------------------------------------------------------

  Scenario: Token sub-tab Overview shows symbol and supply
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    And the token reader knows "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" as "USDC" / "USD Coin" decimals 6 supply 35188571170816
    When the user opens AddressDetail as token for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    Then an "Address" screen is on top
    And once the token probe completes, the Token Overview shows symbol "USDC" and supply 35188571170816

  Scenario: Token Overview surfaces market cap when price is known
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    And the token reader knows "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" as "USDC" / "USD Coin" decimals 6 supply 35000000000000
    And the prices stub returns 1.0001 USD for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    When the user opens AddressDetail with ERC-20 probe for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    Then once the ERC-20 probe completes, the tab bar includes the Token tab
    And the inline Token overview shows symbol "USDC" and price "$1.0001"

  Scenario: Chart sub-tab renders the selected window
    Given the address reader knows contract "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with balance 0 and nonce 1
    And the token reader knows "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" as "USDC" / "USD Coin" decimals 6 supply 35000000000000
    And the prices stub returns 30 points for window "1m" on "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    When the user opens AddressDetail with ERC-20 probe for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    And the user presses "2" to select window "1m"
    Then the active window is "1m"
    And once the feeds complete, the chart holds 30 points for window "1m"

  Scenario: Token Overview flags the unsupported price provider
    Given the address reader knows contract "0xe6a537a407488807f0bbeb0038b79004f19dddfb" with balance 0 and nonce 1
    And the token reader knows "0xe6a537a407488807f0bbeb0038b79004f19dddfb" as "BRLA" / "BRLA Token" decimals 18 supply 1000000000000000000000
    And the Prices API returns 404 for "0xe6a537a407488807f0bbeb0038b79004f19dddfb"
    When the user opens AddressDetail with ERC-20 probe for "0xe6a537a407488807f0bbeb0038b79004f19dddfb"
    Then an "Address" screen is on top
    And once the feeds complete, the Token price row renders "(not indexed by alchemy-prices)"

  Scenario: Non-standard token surfaces the incomplete badge and c jumps to Contract sub-tab
    Given the address reader knows contract "0x0000000000000000000000000000000000009999" with balance 0 and nonce 1
    And the token reader knows "0x0000000000000000000000000000000000009999" as an incomplete non-ERC20 contract
    When the user opens AddressDetail with ERC-20 probe for "0x0000000000000000000000000000000000009999"
    And the user presses "c" to view as contract
    Then the active main tab is "Contract"
