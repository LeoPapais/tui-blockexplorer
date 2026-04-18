---
name: new-use-case
description: Scaffold a new application use case in the blockexplorer-tui repo following the plan-first + BDD + hexagonal workflow. Use when the user asks to add a use case, starts a slice of a plan file, or invokes /new-use-case.
disable-model-invocation: true
---

# Scaffold a new use case

Blockexplorer-tui follows a strict plan-first, BDD-first pipeline. This
skill writes the boilerplate for a new use case so the agent can focus on
the interesting parts: plan text, BDD scenarios, functional assertions
and production code.

## Required inputs

Before touching any file, gather:

1. `USE_CASE` — snake_case name (e.g. `load_address_transfers`).
2. `PLAN_FILE` — which `plan/N-<scope>.md` file already accepts this
   slice. **If none exists, stop and write the plan first.** Refer to
   `.cursor/rules/planning-workflow.mdc`.
3. `SCREEN` — which TUI screen triggers this use case, if any (for the
   BDD feature file). Empty when the use case is purely internal.
4. Port(s) the use case depends on (`TxReaderPort`, `TransfersPort`,
   etc.). Missing ports? Run `/new-port` first.

Ask only for the values you cannot infer from context.

## Files to create

Follow this order — **each file must exist before the next one is
written**, matching the testing.mdc pipeline.

### 1. Plan entry (already exists — only confirm)

Verify that `plan/N-<scope>.md` has a section for this use case
describing input/output, ports, BDD scenarios, deferred pieces. If not,
**abort and ask the user to extend the plan**.

### 2. BDD feature file

`tests/e2e/features/<scope>.feature` — append one scenario per user-
visible behaviour. Use the existing style:

```gherkin
Scenario: <USE_CASE> happy path
  Given <stub primed for the happy path>
  When <the screen triggers USE_CASE>
  Then <the rendered output matches expectation>
```

### 3. Functional test

`tests/functional/<USE_CASE>.rs`:

```rust
//! Functional tests for the `<USE_CASE>` use case.
//!
//! See `plan/<N>-<scope>.md` section <section>.

use blockexplorer_tui::{
    application::use_cases::<USE_CASE>,
    domain::{/* domain types */},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::/* stubs here */;

#[tokio::test]
async fn <one_scenario_name>() {
    // 1) arrange the stubs
    // 2) act: call <USE_CASE>::run(...)
    // 3) assert on the decoded output
    todo!()
}
```

Then wire it into `tests/functional.rs`:

```rust
#[path = "functional/<USE_CASE>.rs"]
mod <USE_CASE>;
```

### 4. Use case module

`src/application/use_cases/<USE_CASE>.rs`:

```rust
//! Use case: <one-line purpose>.
//!
//! See `plan/<N>-<scope>.md` section <section>.

use crate::{
    application::ports::{/* ports */},
    domain::{/* input/output types */, Chain, DomainError},
};

pub async fn run</* generics */>(
    /* port refs + inputs */
) -> Result</* output */, DomainError> {
    // Keep pure: only orchestration of ports.
    todo!()
}
```

Register in `src/application/use_cases/mod.rs`:

```rust
pub mod <USE_CASE>;
```

### 5. Stub (if a new port appeared)

Only if `/new-port` ran beforehand. Otherwise reuse an existing stub.

## Quality gates to run before handing back control

1. `cargo test --test functional <USE_CASE>` — the new scenarios must
   fail for the right reason (red).
2. Implement the use case body so the same command turns green.
3. `cargo test` — no regressions.
4. `cargo clippy --all-targets -- -D warnings` — zero warnings.

## What this skill does NOT do

- It never invents a plan section. If `plan/N-<scope>.md` does not
  mention the use case, refuse to proceed.
- It never short-circuits the test-first step. Every new public
  function in `src/application/use_cases/` ships with a matching
  `tests/functional/<name>.rs`.
- It does not touch adapter code. Port implementations live under
  `src/adapters/` and come with their own wiremock tests; that is
  a separate slice, scaffolded by the plan's Alchemy/Etherscan
  section.

## Output expected of the agent

Before writing any code, the agent should report:

```
Plan:    plan/<N>-<scope>.md section <X>  (verified)
Feature: tests/e2e/features/<scope>.feature  (appended <K> scenarios)
Test:    tests/functional/<USE_CASE>.rs     (NEW, red)
Module:  src/application/use_cases/<USE_CASE>.rs  (NEW, todo!)
Stubs:   <which existing stubs are required>

Proceeding to implement the body. Press Tab to stop.
```

Only then start filling in the implementation.
