# blockexplorer-tui

A terminal block explorer in the spirit of Etherscan. Built in Rust with
`ratatui`, fed by Alchemy, Etherscan V2 and the Sourcify 4byte signature
service. No in-house indexer.

The project is planning-first: every feature is specified under
[`plan/`](plan/README.md) before any code is written. Tests (BDD for user
journeys, TDD for use cases) come before implementation.

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
```

Inside the TUI:

| Key       | Action                                                   |
|-----------|----------------------------------------------------------|
| `q`       | Quit                                                     |
| `Esc`     | Pop current screen (exits when the stack is empty)       |
| `/`       | Open universal search (Home)                             |
| `m`       | Open Mempool (Home)                                      |
| `g`       | Open Gas Tracker (Home)                                  |
| `s`       | Open Settings (Home)                                     |
| `Enter`   | Open selected candidate, transaction or mempool tx       |
| `Up/Down` | Move selection (lists)                                   |
| `Tab`     | Next tab (Block / Tx detail)                             |
| `j`/`k`, Arrows | Scroll tab content by one line (Tx detail)         |
| `PageUp`/`PageDown` | Scroll tab content by ten lines (Tx detail)    |
| `Home`/`End` | Jump to top / bottom of current tab (Tx detail)       |
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
etherscan = "your-etherscan-v2-key"  # optional: enables ABI + event decoding

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

Every MVP screen defined in `plan/*.md` sections 1-10 is wired:
Home, Search, Block detail, Tx detail, Address detail, Contract
detail, Token detail, Mempool, Gas Tracker and Settings — all
reachable from Home with `/`, Enter or a single-letter key (see the
table above). Each screen ships the minimum useful view and
explicitly lists its deferred pieces at the bottom of its plan
file. Mempool opens an empty "waiting..." state in live mode
because the Alchemy WebSocket adapter has not landed yet.
Watchlist, Simulator, Validators and NFT-specific screens remain
out of scope and will arrive as new plan files when prioritised.
