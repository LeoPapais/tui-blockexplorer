Feature: Token detail
  # Mirrors plan/8-token-detail.md section 12.3 (Overview MVP) and
  # section 12.4 (price / transfers / chart).

  Background:
    Given the user is on Home
    And the active chain is "ethereum"

  Scenario: Open a known token
    Given the token reader knows "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" as "USDC" / "USD Coin" decimals 6 supply 35188571170816
    When the user opens TokenDetail for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    Then a "Token" screen is on top
    And once the token is loaded, the Overview shows symbol "USDC" and supply 35188571170816

  Scenario: Open an unknown token falls back gracefully
    When the user opens TokenDetail for "0x0000000000000000000000000000000000000099"
    Then a "Token" screen is on top
    And the Overview stays in the loading state

  Scenario: Token with a spot price surfaces market cap on the Overview
    Given the token reader knows "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" as "USDC" / "USD Coin" decimals 6 supply 35000000000000
    And the prices stub returns 1.0001 USD for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    When the user opens TokenDetail with full feeds for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    Then a "Token" screen is on top
    And once the feeds complete, the Overview price is "$1.0001"

  Scenario: Chart tab renders the selected window
    Given the token reader knows "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" as "USDC" / "USD Coin" decimals 6 supply 35000000000000
    And the prices stub returns 30 points for window "1m" on "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    When the user opens TokenDetail with full feeds for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    And the user presses "2" to select window "1m"
    Then the active window is "1m"
    And once the feeds complete, the chart holds 30 points for window "1m"

  Scenario: Token with no price shows unsupported badge
    Given the token reader knows "0xe6a537a407488807f0bbeb0038b79004f19dddfb" as "BRLA" / "BRLA Token" decimals 18 supply 1000000000000000000000
    And the Prices API returns 404 for "0xe6a537a407488807f0bbeb0038b79004f19dddfb"
    When the user opens TokenDetail with full feeds for "0xe6a537a407488807f0bbeb0038b79004f19dddfb"
    Then a "Token" screen is on top
    And once the feeds complete, the Overview row for price renders "(not indexed by alchemy-prices)"

  Scenario: Transfers tab opens TxDetail on Enter
    Given the token reader knows "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" as "USDC" / "USD Coin" decimals 6 supply 35000000000000
    And the transfers stub returns 1 transfer for token "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" with hash "0xcafe016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a7139cac"
    And the tx reader knows that transfer
    When the user opens TokenDetail with full feeds for "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    And the user switches to the Transfers tab
    And the user presses Enter on the first transfer row
    Then a "Transaction" screen is on top
