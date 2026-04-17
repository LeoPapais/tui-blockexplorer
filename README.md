# blockexplorer-tui

A terminal block explorer in the spirit of Etherscan. Built in Rust with
`ratatui`, fed by Alchemy, Etherscan V2, openchain.xyz and the Samczsun
signature database. No in-house indexer.

The project is planning-first: every feature is specified under
[`plan/`](plan/README.md) before any code is written. Tests (BDD for user
journeys, TDD for use cases) come before implementation.

## Running

```bash
# Frozen view-model, no credentials required:
cargo run -- --demo

# Live view against Alchemy:
ALCHEMY_API_KEY=<your key> cargo run

# Pick a different chain (defaults to Ethereum mainnet):
ALCHEMY_API_KEY=<key> BLOCKEXPLORER_TUI_CHAIN=base cargo run
```

Inside the TUI:

| Key   | Action           |
|-------|------------------|
| `q`   | Quit             |
| `Esc` | Pop current screen (exits when the stack is empty) |

Other keys are reserved and do nothing yet — see the plan files for
the full keymap.

## Configuration file (optional)

`$XDG_CONFIG_HOME/blockexplorer-tui/config.toml`:

```toml
[credentials]
alchemy = "your-key"

[defaults]
chain = "ethereum"  # ethereum | ethereum-sepolia | base | polygon | optimism | arbitrum
```

Environment variables always win over the file.

## Tests

```bash
cargo test                    # unit + functional + e2e
cargo clippy --all-targets -- -D warnings
```

No test performs live network calls: adapters are exercised against
`wiremock`, stubs load JSON fixtures from `tests/fixtures/`.

## Where to look

- [`plan/README.md`](plan/README.md) — index of plan files and the glossary.
- [`plan/0-general-architecture.md`](plan/0-general-architecture.md) —
  navigation philosophy, hexagonal layout, runtime model.
- [`.cursor/rules/`](.cursor/rules) — working rules that apply to every PR
  (planning workflow, architecture, testing, Rust style, external APIs, TUI).

## Status

Home screen is wired end to end (live Alchemy data) and Settings /
Search / Mempool / Block / Tx / Address / Contract / Token / Gas
screens are specified in `plan/` and awaiting implementation.
