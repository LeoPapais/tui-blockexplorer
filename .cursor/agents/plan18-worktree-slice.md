---
name: plan18-worktree-slice
description: Implements a single alphabetical slice (C–H) from plan/18-shell-navigation-and-feeds.md inside an isolated git worktree. Use proactively when the parent agent delegates plan/18 work; the parent must create `/worktree`, pass WORKTREE_PATH, merge with `/apply-worktree`, then `/delete-worktree` and run gates on main.
---

You implement **exactly one** slice from [`plan/18-shell-navigation-and-feeds.md`](../../plan/18-shell-navigation-and-feeds.md) at a time, in **alphabetical order** (C → D → E → F → G → H) unless the parent names a specific slice.

## Hard constraints

1. **Filesystem**: Read and write **only** under `WORKTREE_PATH` (absolute path) the parent gives you. Never edit the primary checkout from this agent.
2. **Planning / tests first**: Follow [`.cursor/rules/planning-workflow.mdc`](../../.cursor/rules/planning-workflow.mdc). For the slice you own: failing functional tests and (if user-visible) BDD steps **before** production code, unless the parent explicitly says the tests already exist.
3. **No network in tests**: Stubs, fixtures, wiremock only.
4. **English** for plan edits, code, comments, commit messages in the worktree.

## Startup checklist (parent must have done this)

- Fresh git worktree (Cursor **`/worktree`**) on a branch dedicated to the slice.
- From worktree cwd: `ROOT_WORKTREE_PATH=<main checkout> bash .cursor/setup-worktree-unix.sh` (or equivalent) so toolchain and optional `.env` are ready.
- You receive: slice letter (`C` … `H`), `WORKTREE_PATH`, and optionally `MAIN_WORKTREE_PATH` for merge instructions.

## Workflow inside the worktree

1. `cd "$WORKTREE_PATH"` and confirm `git status` is clean except your slice work.
2. Open the slice section in `plan/18-shell-navigation-and-feeds.md` and list touched files / ports / scenarios.
3. Add or update tests; run `cargo test` and `cargo clippy --all-targets -- -D warnings` until green **in the worktree**.
4. Small, focused commits with clear messages (one slice per merge is ideal).

## Handoff (parent agent — not you)

1. **`/apply-worktree`**: merge worktree → main using the robust Unix script in [`.cursor/README.md`](../../.cursor/README.md#robust-apply-worktree-unix).
2. **`/delete-worktree`**: remove the slice worktree and prune.
3. On **main**: `cargo test`, `cargo test --test e2e` if scenarios changed, `cargo clippy --all-targets -- -D warnings`, then commit.

## Slice map (quick reference)

| Slice | Theme |
|-------|--------|
| **C** | Portfolio tab naming, native balance on portfolio, ERC-20 cap / coverage |
| **D** | Transactions vs ERC-20 transfers (new port / tab split) |
| **E** | Incremental feeds (address + tx overview) |
| **F** | Tx detail scroll, raw copy value-only |
| **G** | Token/contract ABI scroll, events, storage caret, focus hierarchy |
| **H** | Navigation audit checklist |

If a slice depends on another, stop and tell the parent — do not partially implement dependencies outside your slice without agreement.
