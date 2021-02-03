Feature: API

  Scenario: Check server time
    Given I am checking server time
    Then Get response
    And Validate format
    And Check time

  Scenario: Check server time
    Given I am checking XBT/USD
    Then Get response
    And Validate format
    And Check XBT/USD
