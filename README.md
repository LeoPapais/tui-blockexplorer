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

| Key       | Action                                                   |
|-----------|----------------------------------------------------------|
| `q`       | Quit                                                     |
| `Esc`     | Pop current screen (exits when the stack is empty)       |
| `/`       | Open universal search (Home)                             |
| `m`       | Open Mempool (Home)                                      |
| `Enter`   | Open selected candidate, transaction or mempool tx       |
| `Up/Down` | Move selection (lists)                                   |
| `Tab`     | Next tab (Block / Tx detail)                             |
| `[` / `]` | Previous / next block (Block detail)                     |
| `p`       | Pause / resume stream (Mempool)                          |
| `c`       | Clear list (Mempool)                                     |

Search input accepts tx hashes (0x + 64 hex), block hashes, block numbers,
EVM addresses (0x + 40 hex), ENS names (`*.eth`) and token tickers. Live
mode resolves against Alchemy; demo mode only renders the Home screen.

Other keys are reserved and do nothing yet — see the plan files for the
full keymap.

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

Home, universal Search, Block detail, Transaction detail (MVP) and
Mempool (MVP) are wired end to end. The Mempool screen currently
shows a "waiting..." empty state in live mode because the Alchemy
WebSocket adapter has not landed yet — the screen, filter logic and
pause/clear keybindings are testable end-to-end via stubs in the
BDD suite. Address / Contract / Token detail pages still open as
placeholder screens. Gas Tracker, Settings and the deferred screens
(NFTs, Watchlist, Simulator, Validators) are planned but not yet
built.
