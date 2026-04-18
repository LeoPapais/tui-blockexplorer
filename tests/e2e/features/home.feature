Feature: Home screen
  # Mirrors plan/1-home.md section 7. Every scenario must remain red until
  # the Home use cases are implemented in a later phase.

  Background:
    Given the user launches the app
    And the active chain is "ethereum"

  Scenario: User sees current network stats
    When the Home screen is rendered
    Then the Network card shows the latest block number from the stub
    And the Gas Tracker card shows slow, average and fast gwei values

  Scenario: Gas oracle updates on new block
    Given the Home screen is rendered
    When a new "newHeads" event is pushed from the stub
    Then the Network card updates the latest block number
    And the Gas Tracker card recomputes its values

  Scenario: User switches chain and stats refresh
    Given the Home screen is rendered with "ethereum"
    When the user presses "c"
    And selects "base"
    Then the active chain becomes "base"
    And the Network card reflects the latest block number for "base"

  Scenario: Connection drop shows degraded state
    Given the Home screen is rendered
    When the "newHeads" subscription drops
    Then the header shows a "disconnected" badge
    And the app schedules a reconnect

  Scenario: Connection drop keeps last-known snapshots visible
    Given the Home screen is rendered
    When the "newHeads" subscription drops
    Then the header shows a "reconnecting" hint
    And the Network card still renders the last-known latest block
    And the Gas Tracker card still renders the last-known slow, average and fast gwei
