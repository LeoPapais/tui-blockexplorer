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
| `agents/plan-guard.md` | Readonly subagent that audits a would-be change against the plan-first pipeline. |
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
