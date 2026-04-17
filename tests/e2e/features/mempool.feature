Feature: Mempool
  # Mirrors plan/5-mempool.md section 11.2. The stream is stub-driven;
  # the live Alchemy WebSocket adapter arrives in a later slice.

  Background:
    Given the user is on Home
    And the active chain is "ethereum"

  Scenario: Stream shows three pending txs
    Given the user is on Mempool
    When the stub emits three pending txs
    Then three rows appear in the list

  Scenario: Filter by sender drops non-matching rows
    Given the user is on Mempool
    And the stub emits three pending txs from different senders
    When the filter is set to from "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
    Then only the matching tx remains in the list

  Scenario: Pause blocks new events until resume
    Given the user is on Mempool
    When the stub emits one pending tx
    And the user presses "p"
    And the stub emits two more pending txs
    Then the list still has one row
    When the user presses "p"
    Then the list has three rows

  Scenario: Removed event deletes the row
    Given the user is on Mempool
    And the stub emits one pending tx with hash "0x5a5a000000000000000000000000000000000000000000000000000000000000"
    When the stub emits a removed event for "0x5a5a000000000000000000000000000000000000000000000000000000000000"
    Then the list is empty
