#!/usr/bin/env bash
# Point this clone's git hook path at the repo-tracked `.githooks/`
# directory and make every script under it executable.
#
# Idempotent: re-running is safe. See
# `plan/11-rust-scaffolding.md` §9.5.

set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
hooks_dir="$repo_root/.githooks"

if [[ ! -d "$hooks_dir" ]]; then
    printf 'install-hooks: expected %s to exist.\n' "$hooks_dir" >&2
    exit 1
fi

git -C "$repo_root" config core.hooksPath .githooks
chmod +x "$hooks_dir"/* 2>/dev/null || true

printf 'install-hooks: core.hooksPath now points at .githooks/\n'
printf 'install-hooks: installed hooks:\n'
ls -1 "$hooks_dir" | sed 's/^/  - /'
