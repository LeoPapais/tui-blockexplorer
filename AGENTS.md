# Agent onboarding — blockexplorer-tui

This file is read automatically by Cursor on every agent turn. Keep it
short and link out to deeper docs.

## How this repo works

- **Planning-first.** Every user-visible slice ships after a matching
  section in `plan/*.md`. Read [`plan/README.md`](plan/README.md) for
  the index and the workflow philosophy.
- **Hexagonal.** `src/domain/` has pure types, `src/application/` has
  use cases and outbound ports, `src/adapters/` has HTTP/UI/stub
  implementations, `src/infra/` is the composition root. Cross-layer
  rules live in [`.cursor/rules/architecture.mdc`](.cursor/rules/architecture.mdc).
- **BDD + TDD.** Every use case ships with a functional test under
  `tests/functional/` and, when user-visible, a Cucumber scenario
  under `tests/e2e/features/`. Real HTTP is forbidden in tests —
  stub ports live in `tests/support/stubs.rs` and adapter tests use
  `wiremock`. Details in [`.cursor/rules/testing.mdc`](.cursor/rules/testing.mdc).
- **External APIs.** Alchemy is the RPC primary; Etherscan V2 for
  source/ABI; Sourcify 4byte for signature lookups. See
  [`.cursor/rules/external-apis.mdc`](.cursor/rules/external-apis.mdc).
- **TUI conventions.** `ratatui::widgets::Tabs` with bold + indexed
  background for the active tab; screens never do I/O in `render` or
  `handle_key`; drain channels in `tick`. See
  [`.cursor/rules/tui.mdc`](.cursor/rules/tui.mdc).

## Before writing code

1. Confirm the relevant `plan/{N}-{scope}.md` section exists and is
   accepted. If not, stop and write the plan first.
2. Confirm the use case has a failing functional test (`tests/functional/`).
3. Confirm user-visible flows have a BDD scenario
   (`tests/e2e/features/`).
4. Only then implement.

## Running

```bash
# Demo (no credentials):
cargo run -- --demo

# Live:
ALCHEMY_API_KEY=<k> ETHERSCAN_API_KEY=<k> cargo run

# Tests + lints:
cargo test
cargo clippy --all-targets -- -D warnings
```

## Quality gates per commit

- `cargo test` green (lib + functional + e2e cucumber scenarios).
- `cargo clippy --all-targets -- -D warnings` clean.
- New work documented in the corresponding `plan/*.md`.

## Subagents and worktrees

When the user asks to open a **subagent** for work that touches the
tree, the orchestrator must:

1. Create an isolated checkout with **`/worktree`** (see
   [`.cursor/README.md`](.cursor/README.md#subagents-and-worktrees)).
2. Have the subagent do its work **only** in that worktree path.
3. Merge results back with **`/apply-worktree`** (use the robust Unix
   script in [`.cursor/README.md`](.cursor/README.md#robust-apply-worktree-unix)
   so `git-common-dir` resolution works on the primary checkout), then
   remove the checkout with **`/delete-worktree`** (in that order).
4. On the **primary** worktree (main branch checkout), run
   `cargo test` (and the usual quality gates). If everything passes,
   **commit** the merged result on main.

Skip the worktree dance only when the user explicitly asks for a
read-only subagent (for example `plan-guard`) or when no filesystem
changes are involved.

## Tooling in this repo

- **`.cursor/rules/*.mdc`** — detailed rules per concern (architecture,
  testing, tui, rust-style, planning-workflow, external-apis).
- **`.cursor/worktrees.json`** — sets up new Cursor worktrees
  (`/worktree`, `/best-of-n`) with `.env` copy + `cargo fetch`.
- **`.cursor/hooks.json`** — enforces `cargo fmt`, blocks
  `reqwest::Client::new()` from tests, injects planning context on
  session start.
- **`.cursor/skills/*`** — scaffolding slash commands (`/new-use-case`,
  `/new-port`, `/new-fixture`) that produce boilerplate in the
  repo's canonical shape.
- **`.cursor/agents/plan-guard.md`** — readonly subagent that audits
  a would-be implementation against `plan/` before any code lands.
- **`.cursor/agents/tab-hierarchy-navigation.md`** — implements
  hierarchical tab / subtab / content focus (←/→ tabs, ↑/↓ depth),
  border highlights, and ancestor styling.
- **`.cursor/agents/plan18-worktree-slice.md`** — one alphabetical
  slice (C–H) from `plan/18-shell-navigation-and-feeds.md` per
  isolated **`/worktree`**; parent merges with **`/apply-worktree`**
  then runs gates on main.
- **`.cursor/BUGBOT.md`** — review rules for pull-request-time
  enforcement (delegates to the `.cursor/rules/`).
