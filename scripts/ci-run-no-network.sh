#!/usr/bin/env bash
# Run a command inside a network namespace with no external routes
# (CI contract: tests must not reach the internet). Used by
# `.github/workflows/ci.yml`.
#
# `sudo` is required for `unshare -n` on hosted runners, but it resets
# HOME to /root unless we forward it — otherwise rustup's `cargo`
# tries to populate /root/.rustup and hits DNS (fails in the netns).
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: bash scripts/ci-run-no-network.sh <command> [args...]" >&2
  exit 2
fi

RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"

sudo env \
  "PATH=$PATH" \
  "HOME=$HOME" \
  "USER=${USER:-}" \
  "RUSTUP_HOME=$RUSTUP_HOME" \
  "CARGO_HOME=$CARGO_HOME" \
  "CARGO_INCREMENTAL=${CARGO_INCREMENTAL:-}" \
  "RUSTFLAGS=${RUSTFLAGS:-}" \
  "RUST_BACKTRACE=${RUST_BACKTRACE:-}" \
  "CARGO_TERM_COLOR=${CARGO_TERM_COLOR:-}" \
  unshare -n -- bash -c 'ip link set lo up && exec "$@"' bash "$@"
