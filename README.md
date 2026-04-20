# blockexplorer-tui

A terminal block explorer in the spirit of Etherscan. Built in Rust with
`ratatui`, fed by Alchemy, Etherscan V2 and the Sourcify 4byte signature
service. No in-house indexer.

The project is planning-first: every feature is specified under
[`plan/`](plan/README.md) before any code is written. Tests (BDD for user
journeys, TDD for use cases) come before implementation.

## Key bindings at a glance

| Key                 | Action                                                |
|---------------------|-------------------------------------------------------|
| `/`                 | Universal search                                      |
| `?`                 | Help modal (lists every binding below)                |
| `q` / `Ctrl+C`      | Quit                                                  |
| `Esc`               | Back / close modal                                    |
| `Tab` / `Shift-Tab` | Switch main tab on detail screens                     |
| `[` / `]`           | Switch sub-tab (Contract / Token)                     |
| `1`..`9`            | Jump directly to a sub-tab by number                  |
| Arrows              | Move the field cursor (or list selection)             |
| `Enter`             | Open related screen for the value under the cursor    |
| `y`                 | Copy value under cursor (or the screen's default)     |
| `Y`                 | Copy ENS / canonical identifier                       |
| `e`                 | Export active tab as CSV (Address / Block)            |

Full walkthrough in Portuguese: [`docs/USER_GUIDE.md`](docs/USER_GUIDE.md).

## Running

```bash
# Frozen view-model, no credentials required:
cargo run -- --demo

# Live view against Alchemy:
ALCHEMY_API_KEY=<your key> cargo run

# Add ABI-based method + event decoding on TxDetail (optional but
# recommended):
ALCHEMY_API_KEY=<key> ETHERSCAN_API_KEY=<etherscan-v2-key> cargo run

# Pick a different chain (defaults to Ethereum mainnet):
ALCHEMY_API_KEY=<key> BLOCKEXPLORER_TUI_CHAIN=base cargo run

# Seed ~/.config/blockexplorer-tui/config.toml with defaults and exit
# (valid chain slugs: ethereum | ethereum-sepolia | base | polygon |
# optimism | arbitrum — an invalid BLOCKEXPLORER_TUI_CHAIN surfaces
# the full list in the error message):
cargo run -- --init-config
```

Inside the TUI:

| Key       | Action                                                   |
|-----------|----------------------------------------------------------|
| `q`       | Quit                                                     |
| `Esc`     | Pop current screen (exits when the stack is empty)       |
| `/`       | Open universal search (Home)                             |
| `s`       | Open Settings (Home)                                     |
| `Enter`   | Open selected candidate or transaction                     |
| `Up/Down` | Move selection (lists)                                   |
| `Tab`     | Next tab (Block / Tx / Address detail)                   |
| `j`/`k`, Arrows | Scroll / move selection in the current tab         |
| `PageUp`/`PageDown` | Page scroll / jump 10 rows                     |
| `Home`/`End` | Jump to top / bottom of current tab or list           |
| `[` / `]` | Previous / next block (Block detail)                     |
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
etherscan = "your-etherscan-v2-key"  # optional: enables ABI + event decoding

[defaults]
chain = "ethereum"  # ethereum | ethereum-sepolia | base | polygon | optimism | arbitrum
```

Environment variables always win over the file.

## Tests

```bash
cargo test                    # unit + functional + e2e
cargo clippy --all-targets -- -D warnings
cargo deny check              # supply-chain + license audit (see deny.toml)
```

No test performs live network calls: adapters are exercised against
`wiremock`, stubs load JSON fixtures from `tests/fixtures/`.

## Pre-commit hooks

One-time setup per clone, points `core.hooksPath` at `.githooks/`:

```bash
scripts/install-hooks.sh
```

On every `git commit` the hook runs `cargo fmt --all -- --check` and
`cargo clippy --all-targets -- -D warnings`. Bypass in emergencies with
`git commit --no-verify`; CI will still flag anything the bypass hid.

## User guide

A longer walkthrough of every screen — in Portuguese, matching the
working language of this repo — lives at
[`docs/USER_GUIDE.md`](docs/USER_GUIDE.md). Read it first if you
are git-cloning the repo for the first time; it covers the full
keymap, the cursor model, the search overlay, each detail screen
and the troubleshooting flow for things like `y` silently dropping
in a headless session.

## Where to look

- [`plan/README.md`](plan/README.md) — index of plan files and the glossary.
- [`plan/0-general-architecture.md`](plan/0-general-architecture.md) —
  navigation philosophy, hexagonal layout, runtime model.
- [`.cursor/rules/`](.cursor/rules) — working rules that apply to every PR
  (planning workflow, architecture, testing, Rust style, external APIs, TUI).

## Status

Core screens from `plan/*.md` are wired: Home, Search, Block detail,
Tx detail, unified Address detail (contract + token), and Settings.
**Mempool** and the full-screen **Gas Tracker** were removed (see
`plan/5-mempool.md` and `plan/9-gas-tracker.md` — abandoned); slow /
average / fast gas tiers still appear on the Home gas card only.
Watchlist, Simulator, Validators and NFT-specific screens remain
out of scope and will arrive as new plan files when prioritised.
