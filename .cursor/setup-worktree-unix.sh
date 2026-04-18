#!/usr/bin/env bash
# Runs once per freshly-created Cursor worktree on macOS / Linux.
#
# Schema reference: https://cursor.com/docs/configuration/worktrees
# The script is invoked with the worktree dir as cwd; $ROOT_WORKTREE_PATH
# points at the main checkout so we can pull files / env from it.
#
# Keep this idempotent and fast: the agent waits for it to finish before
# the first turn.

set -euo pipefail

log() { printf "[setup-worktree] %s\n" "$*"; }

log "worktree at $(pwd)"
log "main checkout at ${ROOT_WORKTREE_PATH:-<unset>}"

# ---------------------------------------------------------------------------
# 1. Env file
# ---------------------------------------------------------------------------
# `.env` is in .gitignore (contains ALCHEMY_API_KEY / ETHERSCAN_API_KEY) so it
# does NOT follow the branch. Copy it from the main checkout when present.
if [[ -n "${ROOT_WORKTREE_PATH:-}" && -f "${ROOT_WORKTREE_PATH}/.env" ]]; then
  cp "${ROOT_WORKTREE_PATH}/.env" .env
  log "copied .env from main checkout"
else
  log ".env not present in main checkout (skipped)"
fi

# ---------------------------------------------------------------------------
# 2. Toolchain
# ---------------------------------------------------------------------------
# rust-toolchain.toml pins `stable` + rustfmt + clippy. Touching cargo forces
# rustup to resolve the toolchain on first use so the next cargo invocation is
# snappy.
if command -v cargo >/dev/null 2>&1; then
  cargo --version >/dev/null
  log "rust toolchain: $(cargo --version)"
else
  log "cargo not on PATH — install rustup before running tests"
fi

# ---------------------------------------------------------------------------
# 3. Dependency cache
# ---------------------------------------------------------------------------
# `cargo fetch` downloads registry entries without compiling. Cheap on a
# warm registry (shared via ~/.cargo), keeps the agent's first `cargo check`
# from blocking on network.
if command -v cargo >/dev/null 2>&1; then
  cargo fetch --quiet || log "cargo fetch failed (non-fatal)"
fi

# ---------------------------------------------------------------------------
# 4. Optional warm-up (commented out by default)
# ---------------------------------------------------------------------------
# Uncomment to trade ~30s of setup time for a faster first `cargo test`/
# `cargo check` inside the worktree. Useful for worktrees that will run full
# test suites (/best-of-n with benchmarking, for instance).
#
# cargo check --all-targets --quiet || log "warmup cargo check failed (non-fatal)"

log "ready."
