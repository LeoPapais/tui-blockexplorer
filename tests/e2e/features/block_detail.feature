Feature: Block detail
  # Mirrors plan/3-block-detail.md section 11.3. Scenarios drive the
  # real SearchScreen + BlockDetailScreen + DetailPlaceholderScreen
  # through stubbed ports in the cucumber World.

  Background:
    Given the user is on Home
    And the active chain is "ethereum"

  Scenario: Load block by number through search
    Given the block reader knows block 21345678 with two transactions
    And the stub knows block 21345678
    When the user opens search with "21345678"
    And confirms the block candidate
    Then a "Block" screen is on top
    And once the block is loaded, the Overview tab shows block 21345678

  Scenario: Open a transaction from the block
    Given the user is on BlockDetail for block 21345678 with two transactions
    When the user switches to the Transactions tab
    And selects the first transaction and presses Enter
    Then a "Transaction" screen is on top

  Scenario: Previous and next navigation
    Given the user is on BlockDetail for block 21345678 with two transactions
    And the block reader knows block 21345677 with hash "0xcccc000000000000000000000000000000000000000000000000000000000000"
    And the block reader knows block 21345679 with hash "0xdddd000000000000000000000000000000000000000000000000000000000000"
    When the user presses "["
    Then BlockDetail eventually shows block 21345677
    When the user presses "]"
    Then BlockDetail eventually shows block 21345678
    When the user presses "]"
    Then BlockDetail eventually shows block 21345679
