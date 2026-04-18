---
name: new-fixture
description: Create a new wiremock / stub JSON fixture in tests/fixtures/ following the repo naming convention. Use when adding an adapter scenario, when the user asks to capture an API response, or invokes /new-fixture.
disable-model-invocation: true
---

# Add a new test fixture

Every adapter test in blockexplorer-tui is fed by a static JSON file
under `tests/fixtures/`, loaded via `support::fixture_loader::load_text`.
Never hit real hosts — see `.cursor/rules/testing.mdc`.

## Naming convention

```
tests/fixtures/<provider>__<method>__<case>.json
```

- `provider` — lowercase: `alchemy`, `etherscan`, `sourcify`,
  `ens`, `openchain`, ...
- `method` — the RPC method, REST action, or short descriptor with no
  slashes. Use `_` for multi-word: `asset_transfers`, `token_balances`,
  `getsourcecode`.
- `case` — the scenario: `happy`, `empty`, `revert`, `method_not_found`,
  `verified_single_file`, `multi_file`, etc.

Examples already in the repo:

- `alchemy__eth_call__balanceOf_success.json`
- `alchemy__eth_getLogs__transfer.json`
- `etherscan__getsourcecode__verified_multi_file.json`
- `sourcify__signatures__transfer.json`

## Rules

1. **Keep it minimal.** Drop fields the adapter never reads (e.g. block
   timestamps for a pure RPC echo). Smaller diffs make review easier
   and the semantic index (excluding `tests/fixtures/` via
   `.cursorindexingignore`) stays slim.
2. **Mirror the upstream format exactly**: JSON-RPC envelopes with
   `jsonrpc`, `id`, `result` / `error`; REST shapes mirror the
   provider literally. When in doubt, curl the real API once and
   trim.
3. **Never include secrets.** No API keys, no account identifiers
   tied to the user. Use `0xd8da...` (vitalik), `0xa0b86991...` (USDC)
   for placeholder addresses.
4. **Pair the fixture with a test.** Add an assertion in
   `tests/functional/<adapter>_*.rs` that loads it via
   `load_text(<path>)` and runs the adapter against a `MockServer`.
5. **Document the case in the plan.** The plan's `## Fixtures`
   section lists the canonical cases; extend it if new.

## Common fixture shapes

### JSON-RPC success

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": "0x<hex>"
}
```

### JSON-RPC revert / error

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": 3,
    "message": "execution reverted: InsufficientBalance()"
  }
}
```

### JSON-RPC method-not-found (use for `FeatureUnavailable` tests)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32601,
    "message": "the method <name> does not exist/is not available"
  }
}
```

### Etherscan REST

```json
{
  "status": "1",
  "message": "OK",
  "result": "..."
}
```

## Wiring checklist

- [ ] File at `tests/fixtures/<provider>__<method>__<case>.json`.
- [ ] Functional test loads it via
      `crate::support::fixture_loader::load_text("<name>.json")`.
- [ ] Wiremock `Mock::given(method("POST"))...` returns its body.
- [ ] Plan's "Fixtures" section lists the new case.

## What this skill does NOT do

- It does not fetch the live API response. If you need a faithful
  capture, `curl` manually and trim; never hit the provider from
  inside a test or CI.
- It does not wire a brand-new adapter test file — pair this with the
  existing functional test under `tests/functional/<adapter>.rs`.
