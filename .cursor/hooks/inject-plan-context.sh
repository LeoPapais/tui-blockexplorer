#!/usr/bin/env bash
# sessionStart hook: seed the agent with a compact snapshot of the
# planning state so it knows which plans are done vs. in progress
# before the user even types the first prompt.
#
# We feed back the whole `plan/README.md` (which already has the status
# table) and a terse working-tree hint. Cheap + idempotent.

set -uo pipefail

root="$(pwd)"
PLAN_README="$root/plan/README.md"

context=""
if [[ -f "$PLAN_README" ]]; then
  # Escape \ and " for JSON; collapse newlines to \n.
  snapshot=$(head -200 "$PLAN_README" | sed 's/\\/\\\\/g; s/"/\\"/g' | awk '{printf "%s\\n", $0}')
  context="Plan index snapshot (first 200 lines of plan/README.md):\\n\\n${snapshot}"
fi

branch="$(git -C "$root" rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"
dirty_count="$(git -C "$root" status --porcelain 2>/dev/null | wc -l | tr -d ' ')"

context="${context}\\n\\n---\\nBranch: ${branch}   Uncommitted files: ${dirty_count}"

if [[ -z "$context" ]]; then
  printf '{}'
  exit 0
fi

printf '{"additional_context":"%s"}' "$context"
exit 0
