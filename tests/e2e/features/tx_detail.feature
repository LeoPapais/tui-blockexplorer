Feature: Transaction detail
  # Mirrors plan/4-tx-detail.md section 12.3 (MVP) and section 12.4.2
  # (ABI decoding + Logs tab + Pending support). Scenarios drive the
  # real TxDetailScreen through the stub ports held in the cucumber
  # World.

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

  Scenario: Pending transaction is handled
    Given the tx reader knows tx "0xbeef016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944a" is pending
    When the user opens TxDetail for hash "0xbeef016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944a"
    Then a "Transaction" screen is on top
    And once the transaction is loaded, the Overview tab shows status "pending"

  Scenario: Overview decodes the method via ABI
    Given the tx reader knows tx "0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa" was successful
    And the contract source knows the ABI of "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    When the user opens TxDetail with decoding for hash "0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa"
    Then a "Transaction" screen is on top
    And once the transaction is loaded, the decoded method is "transfer(address,uint256)" from ABI

  Scenario: Unknown selector resolves via openchain
    Given a tx whose target has no verified ABI
    And openchain returns "transfer(address,uint256)" for the selector
    When the user opens TxDetail
    Then the Overview tab shows that signature sourced from the directory

  Scenario: Openchain miss falls back to Samczsun
    Given a tx whose target has no verified ABI
    And openchain returns no match for the selector
    And samczsun returns "transfer(address,uint256)" for the selector
    When the user opens TxDetail
    Then the Overview tab shows that signature with provenance samczsun

  Scenario: Logs tab shows decoded event signatures
    Given the tx reader knows tx "0xcccc016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394cc" emitted a Transfer event
    And the signature directory resolves the Transfer event topic
    When the user opens TxDetail with decoding for hash "0xcccc016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394cc"
    Then a "Transaction" screen is on top
    And once the transaction is loaded, the Logs tab decodes "Transfer(address,address,uint256)"

  Scenario: Asset Changes tab renders simulated deltas
    Given the tx reader knows tx "0xd1d1016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394d1" was successful
    And the simulator reports 1 native ETH transfer for that tx
    When the user opens TxDetail with full enrichment for hash "0xd1d1016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394d1"
    Then a "Transaction" screen is on top
    And once the transaction is loaded, the Asset Changes tab lists 1 change

  Scenario: State Changes tab degrades gracefully when trace is unavailable
    Given the tx reader knows tx "0xd2d2016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394d2" was successful
    And the tracer reports that the state-diff is unsupported
    When the user opens TxDetail with full enrichment for hash "0xd2d2016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394d2"
    Then a "Transaction" screen is on top
    And once the transaction is loaded, the State Changes tab reports it is unsupported

  Scenario: State Changes tab lists address diffs
    Given the tx reader knows tx "0xd3d3016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394d3" was successful
    And the tracer reports a balance diff for the sender
    When the user opens TxDetail with full enrichment for hash "0xd3d3016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394d3"
    Then a "Transaction" screen is on top
    And once the transaction is loaded, the State Changes tab lists 1 address
