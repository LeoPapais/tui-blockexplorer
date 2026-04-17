Feature: Settings
  # Mirrors plan/10-settings.md section 11.1.

  Background:
    Given the user is on Home
    And the active chain is "ethereum"

  Scenario: Open Settings from Home and see Alchemy key status
    When the user opens Settings with an alchemy key configured
    Then a "Settings" screen is on top
    And the Settings screen reports the alchemy key as configured

  Scenario: Open Settings without a key reports missing
    When the user opens Settings with no alchemy key
    Then a "Settings" screen is on top
    And the Settings screen reports the alchemy key as missing
