# Review rules for blockexplorer-tui

Bugbot inspects pull requests and comments on violations. Keep the
enforcement in sync with `.cursor/rules/` so `AGENTS.md`, Bugbot and
the IDE agent all reason with the same rulebook.

Delegate first. For any concern below, the source of truth is one of:

- `.cursor/rules/planning-workflow.mdc`
- `.cursor/rules/architecture.mdc`
- `.cursor/rules/testing.mdc`
- `.cursor/rules/external-apis.mdc`
- `.cursor/rules/rust-style.mdc`
- `.cursor/rules/tui.mdc`

When a PR violates one of those rules, cite it by name and link. Do
not paraphrase — quote the exact bullet.

## Must-flag violations

### Planning

- Any new module under `src/application/use_cases/`,
  `src/application/ports/` or a new screen under
  `src/adapters/ui/` **must** have a matching section in
  `plan/<N>-<scope>.md`. PRs missing the plan entry are blocked.
- Any change claiming a plan slice is `done` must flip the status in
  both the plan file and `plan/README.md`.

### Architecture

- Adapters must map provider errors to `DomainError` **at the adapter
  boundary**. A PR that returns `reqwest::Error`, `serde_json::Error`,
  `EtherscanError`, `RpcError`, etc. across a port signature is
  blocked.
- The `domain` crate never imports from `application`, `adapters` or
  `infra`. The `application` crate never imports from `adapters` or
  `infra`. Flag any such reverse import.
- Ports (traits) live in `src/application/ports/`. A trait defined
  under `src/adapters/` that outside modules depend on is a leak.

### Testing

- **No real HTTP in tests.** Flag `reqwest::Client::new()` inside any
  file under `tests/` that is not paired with `wiremock::MockServer`.
- Every use case in `src/application/use_cases/` must have a
  `tests/functional/<use_case>.rs`; every user-visible screen flow
  must have at least one scenario in `tests/e2e/features/`.
- Every new adapter method must ship with a wiremock test that loads
  a fixture from `tests/fixtures/<provider>__<method>__<case>.json`.

### External APIs

- Adapters must cite the upstream endpoint in a doc comment (which
  provider + which method + link to docs). Flag any new adapter file
  missing that header.
- The Etherscan V2 adapter must pass `chainid=` as query parameter;
  the `EtherscanClient::get` wrapper already does it — do not bypass.
- Signature lookups go to Sourcify 4byte
  (`https://api.4byte.sourcify.dev`). openchain.xyz is no longer the
  primary — do not add it back without updating the rule.

### Rust style

- `.unwrap()` / `.expect()` are allowed in tests, examples, `const`
  initialisers and the composition root (`src/infra/`). Anywhere else
  is a review-blocking smell; demand a `Result` or documented reason.
- New domain types derive `Debug`, `Clone`, `PartialEq`, `Eq` unless
  there is a concrete reason (e.g. contains `f64`).
- `Box<dyn Future>` / `async_trait` must not be reintroduced — every
  port uses `impl std::future::Future`.

### TUI

- Tab bars use `ratatui::widgets::Tabs` with bold + indexed
  background. Reviewers reject re-introducing the `*Overview*` string
  formatter that jittered under selection.
- Screens must not do I/O in `render` or `handle_key`. All background
  work goes through `tick` drain-channel pattern (existing screens
  are the reference).
- Every keyboard handler documents its key in the screen's doc
  comment and, if a new global binding, in `README.md`.

## Autofix scope

Prefer suggestions over autofix. Autofix is **only** acceptable for:

- `cargo fmt` drift
- trivial import ordering
- renaming a single field whose rename is obvious from context

Anything touching tests, plan files, or adapter contracts needs a
human.

## Learned rules

Use `@cursor remember <fact>` in PR comments when you spot a pattern
Bugbot should internalise repo-wide (e.g. "always prefer `AbiValue`
over raw bytes in the Read tab pipeline"). These land under Team
Rules in the dashboard.
