---
name: new-port
description: Scaffold a new outbound port (trait) plus its stub in the blockexplorer-tui repo, following the hexagonal architecture rule. Use when adding a new port, exposing a new external capability, or invoking /new-port.
disable-model-invocation: true
---

# Scaffold a new outbound port

Outbound ports live in `src/application/ports/` and describe what the
application asks of the outside world. Every port has at least one stub
in `tests/support/stubs.rs` used by functional + BDD tests, and a real
adapter under `src/adapters/`.

## Required inputs

1. `PORT_NAME` — PascalCase trait name ending in `Port`, e.g.
   `PricesPort`.
2. `PORT_FILE` — snake_case file name, e.g. `prices`.
3. `METHOD(S)` — async signature(s), return types in domain vocabulary
   (`DomainError` on the failure side).
4. `PLAN_FILE` — which `plan/` section declares this port.

## Files to create

### 1. Plan mention

Confirm that `plan/<N>-<scope>.md` documents the port in its "Ports"
section. Refuse to proceed otherwise (`.cursor/rules/planning-workflow.mdc`).

### 2. Domain types (if new)

Any non-trivial input/output types belong under `src/domain/` first.
Primitives (`Address`, `Chain`, `Wei`) are already there.

### 3. Port module

`src/application/ports/<PORT_FILE>.rs`:

```rust
//! Outbound port: <one-line description of the capability>.
//!
//! Backed by <provider> in live mode. See `plan/<N>-<scope>.md`
//! section <X>.

use crate::domain::{/* types */, Chain, DomainError};

pub trait <PORT_NAME>: Send + Sync {
    fn <method>(
        &self,
        /* inputs */
        chain: Chain,
    ) -> impl std::future::Future<Output = Result</* output */, DomainError>> + Send;
}
```

**Why `impl Future` and not `async fn`:** keeps the trait object-safe
for places that use generics (`impl <PORT_NAME>`) and mirrors every
other port in the repo. Do not diverge.

### 4. Register the module

Append to `src/application/ports/mod.rs`:

```rust
pub mod <PORT_FILE>;
```

And the `pub use` line alphabetically:

```rust
pub use <PORT_FILE>::<PORT_NAME>;
```

### 5. Stub implementation

Append to `tests/support/stubs.rs`:

```rust
// ---------------------------------------------------------------------------
// Stub: <PORT_NAME>
// ---------------------------------------------------------------------------

#[derive(Default)]
struct <PORT_NAME>State {
    // keyed by whatever the port is parametrised on
}

#[derive(Default, Clone)]
pub struct Stub<PORT_NAME> {
    inner: Arc<Mutex<<PORT_NAME>State>>,
}

impl Stub<PORT_NAME> {
    pub fn new() -> Self { Self::default() }

    pub fn set_<case>(&self, /* key */, /* value */) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        // ...
    }
}

impl <PORT_NAME> for Stub<PORT_NAME> {
    async fn <method>(
        &self,
        /* inputs */,
        _chain: Chain,
    ) -> Result</* output */, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        // return primed value
        todo!()
    }
}
```

Also update the imports at the top of `tests/support/stubs.rs` to
include the new port and any new domain types.

### 6. Stub registration in test world

If any BDD scenario uses this port, add a field on `AppWorld` in
`tests/e2e/world.rs`:

```rust
pub <port_file>_stub: Stub<PORT_NAME>,
```

## What this skill does NOT do

- It does not write the live adapter under `src/adapters/rpc/` or
  `src/adapters/etherscan/`. That is a separate slice of the plan,
  with its own wiremock functional test.
- It does not implement the use case that consumes the port. Run
  `/new-use-case` for that.
- It does not decide the API shape — the shape comes from the plan.

## Output expected of the agent

```
Plan:    plan/<N>-<scope>.md (port mentioned in Ports section)
Domain:  <new types, if any>
Port:    src/application/ports/<PORT_FILE>.rs  (NEW)
Stub:    Stub<PORT_NAME> appended to tests/support/stubs.rs
World:   tests/e2e/world.rs -- added <port_file>_stub field (if needed)

Ready to wire the use case via /new-use-case <name>.
```
