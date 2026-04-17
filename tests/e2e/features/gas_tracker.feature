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
