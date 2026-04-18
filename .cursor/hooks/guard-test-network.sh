#!/usr/bin/env bash
# beforeShellExecution hook: before running `cargo test` (or nextest),
# scan the *modified* test files for real-HTTP smells that violate
# .cursor/rules/testing.mdc.
#
# Fail-open philosophy: we only *warn* the agent (permission=ask) so
# humans can override; we never hard-block, because false positives
# inside commented-out code would be obnoxious.

set -uo pipefail

input=$(cat)

command=""
if command -v jq >/dev/null 2>&1; then
  command=$(printf '%s' "$input" | jq -r '.command // empty' 2>/dev/null || echo "")
fi
if [[ -z "$command" ]]; then
  command=$(printf '%s' "$input" \
    | grep -oE '"command":\s*"[^"]+"' \
    | head -1 \
    | sed -E 's/.*"command":\s*"([^"]+)".*/\1/')
fi

if [[ -z "$command" ]]; then
  printf '{"permission":"allow"}'
  exit 0
fi

# Scan every .rs file that lives under tests/ for literal RPC URLs.
# Stub ports + wiremock fixtures never mention real hosts; hits here
# usually mean the agent forgot to swap in a MockServer.
OFFENDERS=""
if [[ -d tests ]]; then
  while IFS= read -r line; do
    OFFENDERS="${OFFENDERS}${line}"$'\n'
  done < <(grep -RInE \
    'https?://([a-zA-Z0-9-]+\.)*(alchemy|etherscan|sourcify|infura|quicknode|ankr|public-node|blockpi|4byte)\b' \
    tests 2>/dev/null || true)
fi

if [[ -n "$OFFENDERS" ]]; then
  # Truncate to keep the JSON payload reasonable.
  excerpt=$(printf '%s' "$OFFENDERS" | head -20 | sed 's/"/\\"/g' | awk '{printf "%s\\n", $0}')
  printf '{
    "permission": "ask",
    "user_message": "`%s` will run, but tests reference real RPC hosts (see .cursor/rules/testing.mdc — no network in tests). Review before continuing.",
    "agent_message": "Hook flagged probable real-HTTP calls in tests/ before running `%s`. Offenders:\\n%s\\nSwap to MockServer / stub ports before calling cargo test."
  }' "$command" "$command" "$excerpt"
  exit 0
fi

printf '{"permission":"allow"}'
exit 0
