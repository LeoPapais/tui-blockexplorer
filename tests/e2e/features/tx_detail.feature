Feature: Transaction detail
  # Mirrors plan/4-tx-detail.md section 12.3 (MVP scope: Overview +
  # Raw tabs). Scenarios drive the real TxDetailScreen through the
  # StubTxReaderPort held in the cucumber World.

  Background:
    Given the user is on Home
    And the active chain is "ethereum"

  Scenario: Open a successful tx from BlockDetail
    Given the tx reader knows tx "0x0000000000000000000000000000000000000000000000000000000000000000" was successful
    And the user is on BlockDetail for block 21345678 with two transactions
    When the user switches to the Transactions tab
    And selects the first transaction and presses Enter
    Then a "Transaction" screen is on top
    And once the transaction is loaded, the Overview tab shows status "success"

  Scenario: Reverted transaction shows reason
    Given the tx reader knows tx "0xfefe016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944f" failed with reason "InsufficientBalance()"
    When the user opens TxDetail for hash "0xfefe016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944f"
    Then a "Transaction" screen is on top
    And once the transaction is loaded, the Overview tab shows status "failed - InsufficientBalance()"
