# blockexplorer-tui

A terminal block explorer in the spirit of Etherscan. Built in Rust with
`ratatui`, fed by Alchemy, Etherscan V2, openchain.xyz and the Samczsun
signature database. No in-house indexer.

The project is planning-first: every feature is specified under
[`plan/`](plan/README.md) before any code is written. Tests (BDD for user
journeys, TDD for use cases) come before implementation.

## Where to look

- [`plan/README.md`](plan/README.md) — index of plan files and the glossary.
- [`plan/0-general-architecture.md`](plan/0-general-architecture.md) —
  navigation philosophy, hexagonal layout, runtime model.
- [`.cursor/rules/`](.cursor/rules) — working rules that apply to every PR
  (planning workflow, architecture, testing, Rust style, external APIs, TUI).

## Status

Pre-implementation. Only planning artefacts exist in the repo so far.
