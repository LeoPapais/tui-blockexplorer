#!/usr/bin/env bash
# Enforce the module-boundary rules documented in
# `.cursor/rules/architecture.mdc` with a lightweight grep-based
# scanner. Exits non-zero on any violation.
#
# Rules:
#   1. `src/domain/**`            must not import
#      `crate::{application, adapters, infra}` or their
#      `blockexplorer_tui::` equivalents.
#   2. `src/application/**`       must not import
#      `crate::{adapters, infra}`.
#   3. `src/adapters/ui/**`       must not import any sibling adapter
#      module (rpc, etherscan, signatures, ens, cache, prices, labels,
#      secrets, config). UI depends on application ports only.
#
# See `plan/11-rust-scaffolding.md` §9.7.

set -uo pipefail

ROOT="${ROOT:-$(pwd)}"
violations=0

# A convenience wrapper. `grep -nE` with exit-code-aware output: if
# grep finds nothing, we silently move on; if it finds matches we
# print them *and* flip the violation counter.
scan() {
    local pattern="$1"
    local scope="$2"
    local label="$3"

    if [[ ! -d "$scope" ]]; then
        return
    fi

    local hits
    hits=$(grep -rnE --include='*.rs' "$pattern" "$scope" 2>/dev/null || true)
    if [[ -n "$hits" ]]; then
        printf '\n[check-layers] %s\n' "$label"
        printf '%s\n' "$hits" | sed 's/^/  /'
        violations=$((violations + 1))
    fi
}

cd "$ROOT"

# Rule 1: domain -> nothing internal.
#
# We scan `use` statements only. Doc-comment references to
# `crate::application::Foo` are legitimate and should not trip the
# script; actual imports do. Two forms are checked:
#
#   use crate::application::...
#   use crate::{application, ...}
scan \
    '^\s*(pub\s+)?use\s+(crate|blockexplorer_tui)::(application|adapters|infra)\b' \
    "src/domain" \
    "domain/ imports from application/ adapters/ or infra/ (rule 1)"
scan \
    '^\s*(pub\s+)?use\s+(crate|blockexplorer_tui)::\{[^}]*\b(application|adapters|infra)\b' \
    "src/domain" \
    "domain/ nested-use imports application/ adapters/ or infra/ (rule 1)"

# Rule 2: application -> only domain.
scan \
    '^\s*(pub\s+)?use\s+(crate|blockexplorer_tui)::(adapters|infra)\b' \
    "src/application" \
    "application/ imports from adapters/ or infra/ (rule 2)"
scan \
    '^\s*(pub\s+)?use\s+(crate|blockexplorer_tui)::\{[^}]*\b(adapters|infra)\b' \
    "src/application" \
    "application/ nested-use imports adapters/ or infra/ (rule 2)"

# Rule 3: adapters::ui -> no sibling adapters.
scan \
    '^\s*(pub\s+)?use\s+(crate|blockexplorer_tui)::adapters::(rpc|etherscan|signatures|ens|cache|prices|labels|secrets|config|rng|clock)\b' \
    "src/adapters/ui" \
    "adapters/ui/ imports from sibling adapter modules (rule 3)"
scan \
    '^\s*(pub\s+)?use\s+(crate|blockexplorer_tui)::adapters::\{[^}]*\b(rpc|etherscan|signatures|ens|cache|prices|labels|secrets|config|rng|clock)\b' \
    "src/adapters/ui" \
    "adapters/ui/ nested-use imports sibling adapter modules (rule 3)"

if [[ "$violations" -ne 0 ]]; then
    printf '\n[check-layers] %d layer-rule violation group(s) found.\n' "$violations" >&2
    printf '[check-layers] See .cursor/rules/architecture.mdc for the full contract.\n' >&2
    exit 1
fi

printf '[check-layers] ok — no cross-layer import violations.\n'
exit 0
