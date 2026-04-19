Feature: Gas Tracker
  # Mirrors plan/9-gas-tracker.md section 11.1.

  Background:
    Given the user is on Home
    And the active chain is "ethereum"

  Scenario: Press g from Home to open the Gas Tracker
    Given the gas stub snapshot has slow 12 average 14 fast 18
    When the user opens the Gas Tracker
    Then a "Gas Tracker" screen is on top
    And the Gas Tracker shows slow 12 average 14 fast 18

  Scenario: Pause halts further updates
    # plan/9-gas-tracker.md §11.2.
    Given the gas stub snapshot has slow 12 average 14 fast 18
    And the user opens the Gas Tracker
    When the user presses "p" on the Gas Tracker
    And the gas feed emits a snapshot with base fee 99
    Then the Gas Tracker badge shows "paused"
    And the Gas Tracker base fee is still the primed value

  Scenario: Ctrl+R forces an immediate refresh
    # plan/9-gas-tracker.md §11.2.
    Given the gas stub snapshot has slow 12 average 14 fast 18
    And the user opens the Gas Tracker with a refresh handle
    When the user presses "Ctrl+R" on the Gas Tracker
    Then a refresh kick has been queued on the handle

  Scenario: Histogram shows p25/p50/p75
    # plan/9-gas-tracker.md §11.3.
    Given the gas stub snapshot has slow 12 average 14 fast 18
    And the user opens the Gas Tracker
    When the gas feed emits base fees 10, 20, 30, 40, 50
    Then the Gas Tracker histogram reads p25 20 p50 30 p75 40

  Scenario: Open converter with u and convert 1 ether to gwei
    # plan/9-gas-tracker.md §11.4.
    Given the gas stub snapshot has slow 12 average 14 fast 18
    And the user opens the Gas Tracker
    When the user presses "u" on the Gas Tracker
    And the user types "1" into the unit converter
    And the user submits the unit converter
    Then the Gas Tracker converter result is "1000000000"
