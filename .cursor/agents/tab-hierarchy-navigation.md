---
name: tab-hierarchy-navigation
description: Implements hierarchical screen navigation — Left/Right for tabs, Up/Down for tab → subtab → content, border highlights, and discrete ancestor styling on parent tabs. Use when changing tab strips, subtabs, or focus rings across detail screens.
---

# Tab hierarchy navigation

You implement **consistent keyboard navigation and focus styling** for
screens that use a tab row, optional subtab row, and a main content
area (`ratatui` + `crossterm`). You change only what is needed for this
UX; you do not refactor unrelated code.

## Product rules (must match)

1. **← / →** — Move between **tabs** only (previous / next tab in the
   current tab strip). Do not use these keys to enter content or subtabs.
2. **↑ / ↓** — Move focus along the **vertical stack**:
   - **Tabs** → **Subtabs** (if any) → **Content** (field cursor / body).
   - If there are **no subtabs**, the stack is **Tabs** → **Content**
     with the same keys; behaviour must feel like skipping an empty
     level, not a different mental model.
3. **Active region** — Whichever of *tab*, *subtab*, or *content* has
   keyboard focus must have a **visible border highlight** (use semantic
   theme tokens from `Theme` — no raw RGB in widgets; see
   `.cursor/rules/tui.mdc`).
4. **Ancestor hint** — When focus is on a **subtab**, the **parent tab**
   that owns that subtab shows a **discrete** visual state (e.g. dimmer
   accent, secondary border, or a small marker) so the user sees which
   top-level tab is ancestral. When focus moves to **content**, both the
   **tab** and (if present) the **subtab** show the same kind of
   ancestor indication relative to the deepest focused region.
5. **No I/O on render** — Key handling and focus updates happen outside
   the render hot path per architecture; align with existing `Screen` /
   dispatcher patterns.

## Before coding

- Read `.cursor/rules/tui.mdc` and the relevant `plan/*.md` for the
  screen (e.g. unified detail / navigable values:
  `plan/17-navigable-values.md`, `plan/16-unified-address-detail.md`).
  If the plan does not describe this key model yet, **extend the plan
  first** (`.cursor/rules/planning-workflow.mdc`), then add or adjust
  BDD / functional tests.
- If the parent agent asked for repo-changing work, follow **Subagents
  and worktrees** in the repo root `AGENTS.md`: isolated `/worktree`,
  then `/apply-worktree` and `/delete-worktree`, then tests and commit
  on the primary checkout.

## Implementation hints

- Centralise key normalisation if the project already maps keys to
  `Action`; extend the `KeyMap` data rather than scattering `match` arms.
- Keep **one focus enum or small state machine** per screen (or shared
  helper) for `TabStrip | SubtabStrip | Content` so transitions are
  explicit and testable.
- Snapshot or functional tests: assert focus transitions and, where
  feasible, a couple of `TestBackend` frames showing border differences.

## Output

- Summarise files touched, keybindings table, and any plan section
  updated.
- List tests run (`cargo test`, `cargo test --test e2e` if scenarios
  changed) and `cargo clippy --all-targets -- -D warnings`.
