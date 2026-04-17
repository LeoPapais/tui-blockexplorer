# 14 — Config, credentials and Home feed

Phase 3 of the Option B roadmap. Brings credentials + live Alchemy
refresh to the Home screen. At the end of this phase
`ALCHEMY_API_KEY=... cargo run` shows live Ethereum data in the terminal
and `cargo run -- --demo` still works as a zero-dependency fallback.

## 1. Purpose and scope

- Load `ALCHEMY_API_KEY` and the default chain from the environment and
  (optionally) from a config file.
- Extend `HomeScreen` with a channel-based feed so the runtime can push
  fresh view-model snapshots without the screen ever touching a port.
- Spawn a background task in `infra::run` that drives `HomeSession`
  against the real Alchemy adapters and pushes the resulting view model
  into the feed every N seconds.
- Make `cargo run` use real data when a key is present, fall back to
  `--demo` otherwise.

Out of scope:

- A full-featured Settings screen: the Settings plan
  (`plan/10-settings.md`) is not delivered here; only the minimum config
  plumbing necessary to locate the API key.
- WebSocket / newHeads subscription.
- Chain picker UX: the active chain comes from config and cannot yet be
  changed at runtime (the `switch_chain` use case is already in place
  but unused by the binary).
- OS keychain integration.

## 2. Configuration

### 2.1 Sources, in order

1. Environment variables (`ALCHEMY_API_KEY`, `BLOCKEXPLORER_TUI_CHAIN`).
2. Config file at `$XDG_CONFIG_HOME/blockexplorer-tui/config.toml`
   (resolved via the `directories` crate).
3. Defaults: no key, chain `ethereum`.

The env vars win if set.

### 2.2 Types

New module `src/infra/config.rs`:

```rust
pub struct AppConfig {
    pub credentials: ApiCredentials,
    pub chain: Chain,
}

pub struct ApiCredentials {
    pub alchemy: Option<String>,
}

impl AppConfig {
    pub fn load() -> Result<Self, DomainError>;
    pub fn has_alchemy_key(&self) -> bool;
}
```

`AppConfig::load` reads env first, then the file (if it exists), then
merges. Missing file is not an error; malformed file is.

### 2.3 File format

```toml
[credentials]
alchemy = "your-key-here"

[defaults]
chain = "ethereum"
```

Every field is optional. Unknown sections are ignored.

## 3. Home feed

### 3.1 Types

In `src/adapters/ui/home.rs`:

```rust
pub struct HomeFeed {
    rx: tokio::sync::mpsc::UnboundedReceiver<HomeViewModel>,
}

impl HomeScreen {
    pub fn with_feed(initial: HomeViewModel, feed: HomeFeed) -> Self;
}
```

The screen holds the feed directly. On every `tick`, it drains the
receiver non-blockingly (`rx.try_recv`) and updates its internal view
model with the latest value it sees. No port call from the render path.

### 3.2 Producer side

`src/infra/home_feed.rs` exposes:

```rust
pub struct HomeFeedSender { tx: UnboundedSender<HomeViewModel> }

pub fn start<N, G, C>(
    session: HomeSession<N, G, C>,
    period: Duration,
    runtime: &Handle,
) -> (HomeFeed, JoinHandle<()>);
```

The spawned task runs a loop:

```
loop {
    session.refresh().await;         // ignores Err(DomainError::*)
    tx.send(session.view().clone());
    tokio::time::sleep(period).await;
}
```

Errors do not break the loop: the session already marks itself as
disconnected on `ProviderUnavailable`, so the view model reflects the
state naturally.

## 4. Runtime wiring

`src/infra/mod.rs::run` becomes:

```
1. Parse CLI + env.
2. Decide data source:
   a. --demo -> frozen view-model (existing path).
   b. else if config.has_alchemy_key() -> real adapters.
   c. else -> print hint, exit 0.
3. When using real adapters:
   - Build reqwest::Client once.
   - Build RpcClient targeted at the active chain's Alchemy URL.
   - Build Alchemy{NetworkStatus,GasOracle}Adapter + StubChainRegistry
     (the chain registry still comes from hardcoded config for now;
     a real one arrives with the Settings screen).
   - Build HomeSession; start HomeFeed; build HomeScreen with it.
4. Run the event loop.
```

## 5. Tests

- `tests/functional/config_load.rs`: env variables set -> picked up;
  config file parsed; env beats file; missing file is not an error;
  malformed file yields `DomainError::Config`.
- `tests/functional/home_feed.rs`: the HomeScreen starts with the
  initial view, receives a feed update via the channel, drains it on
  tick, render reflects the new value.

No new BDD scenarios yet. The Home feature file remains stub-driven
because the e2e harness does not need a real runtime loop to exercise
the screen.

## 6. Acceptance

- `cargo test`: 35+ functional tests green; previous e2e green.
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo run`: prints the hint when no key is set (current behaviour).
- `ALCHEMY_API_KEY=xxxxx cargo run`: enters the TUI, calls Alchemy,
  shows the real head block and gas tiers for Ethereum. Pressing `q`
  exits cleanly.
- `cargo run -- --demo`: still opens the frozen view, regardless of env.

## 7. Follow-up

- `plan/10-settings.md` materialises the Settings screen (theme,
  keybinds, chain picker, cache).
- `plan/15-websocket-subscriptions.md` (future) replaces polling with
  newHeads subscriptions.
