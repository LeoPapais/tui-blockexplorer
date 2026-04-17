Feature: Token detail
  # Mirrors plan/8-token-detail.md section 12.3. Scenarios drive the
  # TokenDetailScreen through StubTokenReaderPort.

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
