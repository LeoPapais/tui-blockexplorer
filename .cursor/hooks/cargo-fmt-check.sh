#!/usr/bin/env bash
# afterFileEdit hook: if a .rs file was just written, verify it is
# rustfmt-clean and ask the agent to format it if not.
#
# Contract: read JSON on stdin, emit JSON on stdout. The matcher in
# .cursor/hooks.json narrows to Write, but we still re-filter by path
# extension so edits to markdown/json pass through untouched.

set -uo pipefail

# Cursor feeds JSON like {"tool":"Write","input":{"path":"..."}}. jq is
# the only strict dependency; we fall back to a grep-based extractor if
# jq is unavailable.
input=$(cat)

path=""
if command -v jq >/dev/null 2>&1; then
  path=$(printf '%s' "$input" | jq -r '.input.path // .input.file_path // empty' 2>/dev/null || echo "")
fi
if [[ -z "$path" ]]; then
  path=$(printf '%s' "$input" \
    | grep -oE '"(path|file_path)":\s*"[^"]+"' \
    | head -1 \
    | sed -E 's/.*"(path|file_path)":\s*"([^"]+)".*/\2/')
fi

# Only act on Rust source files inside the repo.
if [[ "$path" != *.rs ]]; then
  printf '{}'
  exit 0
fi

if ! command -v rustfmt >/dev/null 2>&1; then
  # rustfmt missing on this machine: fail open with a note.
  printf '{"additional_context":"rustfmt is not installed; skip cargo fmt check."}'
  exit 0
fi

# Check only the edited buffer. Passing the path directly would make
# rustfmt follow `mod` declarations (so editing src/lib.rs would scan
# the whole crate); piping through stdin scopes the check to this one
# file.
if ! [[ -f "$path" ]]; then
  printf '{}'
  exit 0
fi

# `rustfmt --check` over stdin always exits 0 (historical bug); we
# read stdout instead: empty output means "already formatted", any
# non-empty diff means "dirty". Using stdin keeps the check scoped to
# the edited buffer (passing the path makes rustfmt follow `mod`
# declarations and cascade to other files).
diff_output=$(rustfmt --check --edition 2024 --quiet < "$path" 2>&1)
if [[ -z "${diff_output// }" ]]; then
  printf '{}'
  exit 0
fi

# Not formatted: tell the agent to run rustfmt. We use additional_context
# instead of a hard block because the agent may have intentional
# mid-edit state; we just want it to know before moving on.
diff_excerpt=$(printf '%s' "$diff_output" | head -40 | sed 's/\\/\\\\/g; s/"/\\"/g' | awk '{printf "%s\\n", $0}')
printf '{"additional_context":"rustfmt --check failed for %s. Run `rustfmt --edition 2024 %s` before declaring the edit done. Excerpt:\\n%s"}' \
  "$path" "$path" "$diff_excerpt"
exit 0
