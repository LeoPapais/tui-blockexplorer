# `.cursor/` overview

Everything the Cursor agent reads on every turn lives here. None of it
touches production code — it only shapes how the agent plans, tests
and reviews.

## Files

| Path | Purpose |
|---|---|
| `rules/*.mdc` | Source of truth for architecture / testing / TUI / planning / external API / Rust style rules. Always applied. |
| `hooks.json` + `hooks/*.sh` | Enforcement: rustfmt check on write, network-call guard on `cargo test`, plan snapshot on session start. |
| `skills/*/SKILL.md` | Invocable slash commands: `/new-use-case`, `/new-port`, `/new-fixture`. |
| `agents/*.md` | Project subagents (`plan-guard`, `tab-hierarchy-navigation`, …). |
| `BUGBOT.md` | Review rules applied to pull requests (delegates to `rules/`). |
| `mcp.json` | MCP servers made available to the agent (currently: `fetch`). |
| `worktrees.json` + `setup-worktree-unix.sh` | Setup run for each new `/worktree` or `/best-of-n` invocation. |

## One-time setup

### MCP `fetch` server

Used so the agent can pull upstream docs (Alchemy, Etherscan V2,
Sourcify 4byte, EIP specs) on demand. The repo's `mcp.json` invokes
the Python-based server via `uvx`, which installs the package on
first use and keeps it in an isolated environment.

One-time install of `uv` (the Python package runner):

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
```

After that Cursor auto-starts `uvx mcp-server-fetch` whenever the
agent needs a `fetch_url` tool — no manual `pip install`.

If you'd rather skip `uv` entirely, swap `mcp.json` to:

```json
{
  "mcpServers": {
    "fetch": {
      "command": "python3",
      "args": ["-m", "mcp_server_fetch"]
    }
  }
}
```

and run `pip install --user mcp-server-fetch` once.

### Hook scripts

Already executable. Re-chmod if they ever lose the `+x` bit after a
move:

```bash
chmod +x .cursor/hooks/*.sh .cursor/setup-worktree-unix.sh
```

Hook scripts depend on `bash`, `rustfmt`, `git`, and `jq`. All four
are already required by the project's test suite, so no extra
installs.

## Subagents and worktrees

When the user asks to **open a subagent** for implementation or other
repo-changing work, the parent agent must keep the main checkout clean
of concurrent edits by using a dedicated git worktree for the
subagent’s session:

1. **`/worktree`** — create a detached worktree (see Cursor command
   docs) and run `worktrees.json` setup. Record `WORKTREE_PATH` and use
   it for all reads, edits, and git commands for that task until merge.
2. **Subagent work** — only inside `WORKTREE_PATH`.
3. **`/apply-worktree`** — copy merged changes from the worktree into
   the primary checkout (main). Resolve conflicts on main if needed.
   Use the **Unix script below** (not a bare `cd` into `git-common-dir`
   from an arbitrary cwd: on the primary checkout Git often reports
   `.git` as a **relative** path, which breaks the naive `cd "$(git …
   rev-parse --git-common-dir)"` check).
4. **`/delete-worktree`** — remove the temporary worktree and prune.
5. **Primary checkout** — run `cargo test` and `cargo clippy --all-targets -- -D warnings`
   (same gates as [AGENTS.md](../AGENTS.md)). If green, **commit** on
   the branch the user expects (usually `main`).

Do **not** use this pipeline for readonly subagents (for example
`plan-guard`) or when the task explicitly does not touch tracked files.

### Robust `/apply-worktree` (Unix)

Substitute `SOURCE_WORKTREE_PATH` and `MAIN_WORKTREE_PATH`, then run:

```bash
set -euo pipefail
SOURCE_WORKTREE_PATH="<source>"
MAIN_WORKTREE_PATH="<main>"
SOURCE_ABS="$(cd "$SOURCE_WORKTREE_PATH" && pwd -P)"
MAIN_ABS="$(cd "$MAIN_WORKTREE_PATH" && pwd -P)"
# Resolve the shared object database from *inside* each tree so relative
# `.git` from `rev-parse --git-common-dir` is unambiguous.
_git_common_abs() {
  (
    cd "$1"
    if git rev-parse --path-format=absolute --git-common-dir >/dev/null 2>&1; then
      git rev-parse --path-format=absolute --git-common-dir
    else
      realpath "$(git rev-parse --git-common-dir)"
    fi
  )
}
SOURCE_COMMON="$(_git_common_abs "$SOURCE_ABS")"
MAIN_COMMON="$(_git_common_abs "$MAIN_ABS")"
if [ "$SOURCE_COMMON" != "$MAIN_COMMON" ]; then
  echo "ERROR: source and main do not share the same repository." >&2
  exit 1
fi
UNTRACKED_FILES="$(git -C "$SOURCE_ABS" ls-files --others --exclude-standard)"
HAS_TRACKED=false
if [ -n "$(git -C "$SOURCE_ABS" diff --name-only HEAD)" ] ||
   [ -n "$(git -C "$SOURCE_ABS" diff --cached --name-only)" ]; then
  HAS_TRACKED=true
fi
if [ "$HAS_TRACKED" = false ] && [ -z "$UNTRACKED_FILES" ]; then
  echo "Nothing to apply."
  exit 0
fi
if [ "$HAS_TRACKED" = true ]; then
  git -C "$SOURCE_ABS" add -u -- .
  git -C "$SOURCE_ABS" commit -m "tmp: worktree apply snapshot" --no-verify --allow-empty
  TEMP_COMMIT="$(git -C "$SOURCE_ABS" rev-parse HEAD)"
  git -C "$MAIN_ABS" cherry-pick --no-commit "$TEMP_COMMIT" || true
  git -C "$MAIN_ABS" reset HEAD -- . >/dev/null 2>&1 || true
  git -C "$SOURCE_ABS" reset --mixed HEAD~1 >/dev/null 2>&1
fi
if [ -n "$UNTRACKED_FILES" ]; then
  echo "$UNTRACKED_FILES" | while IFS= read -r f; do
    [ -z "$f" ] && continue
    mkdir -p "$(dirname "$MAIN_ABS/$f")"
    cp "$SOURCE_ABS/$f" "$MAIN_ABS/$f"
  done
fi
git -C "$MAIN_ABS" status --short
echo "MAIN_WORKTREE=$MAIN_ABS"
echo "SOURCE_WORKTREE=$SOURCE_ABS"
```

If `git cherry-pick` aborts because the primary checkout has local
changes, run `git stash push -u` on **main** first, repeat the
`HAS_TRACKED` block, then `git stash pop` and resolve overlaps.

## Daily usage

- `/new-use-case`, `/new-port`, `/new-fixture` — scaffold the matching
  files end-to-end (plan → BDD → functional → module/stub/fixture).
- `/worktree <task>` — isolated worktree (uses `worktrees.json`).
- `/best-of-n <model-a>, <model-b> <task>` — parallel attempts across
  models, each in its own worktree.
- Call the `plan-guard` subagent before starting any implementation.
  It is read-only and fast — two seconds of validation beats a 30-min
  rewrite.

## Rule precedence (reminder)

Team Rules > Project Rules (`.cursor/rules/`) > User Rules. `AGENTS.md`
on the repo root is read by every agent in every directory. Bugbot
further layers `BUGBOT.md` on top at PR review time.
