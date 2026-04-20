Feature: Home screen
  # Mirrors plan/1-home.md section 7. Every scenario must remain red until
  # the Home use cases are implemented in a later phase.

  Background:
    Given the user launches the app
    And the active chain is "ethereum"

  Scenario: User sees current network stats
    When the Home screen is rendered
    Then the Network card shows the latest block number from the stub
    And the Gas card shows slow, average and fast gwei values

  Scenario: Gas oracle updates on new block
    Given the Home screen is rendered
    When a new "newHeads" event is pushed from the stub
    Then the Network card updates the latest block number
    And the Gas card recomputes its values

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
    And the Gas card still renders the last-known slow, average and fast gwei

  Scenario: New head event updates the Home view
    Given the Home screen is rendered
    When a new head is received from the "newHeads" subscription
    Then the Network card updates the latest block number
    And the Gas card recomputes its values

  Scenario: Esc on Home keeps the app running
    # See plan/12-screen-runtime.md §7.1: Pop on a single-screen stack is a
    # no-op so Esc on Home cannot close the app by accident.
    Given the user is on Home
    When the user presses Esc on Home
    Then Home is still on top of the stack
    And the app is still running

  Scenario: q on Home quits the app
    # Regression guard for the only remaining exit path bound to the
    # keyboard: q (and, by mirroring, Ctrl+C / SIGINT).
    Given the user is on Home
    When the user presses q on Home
    Then the app is no longer running

  Scenario: First-run banner invites the user to Settings
    Given the Home screen is rendered without an alchemy key
    Then the Home screen shows the first-run credentials banner
    When the user dismisses the first-run banner with Esc
    Then the Home screen no longer shows the first-run credentials banner
