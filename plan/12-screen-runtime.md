# 12 — Screen runtime

Phase 1 of the Option B roadmap from the project chat. Wires the TUI so
that `cargo run -- --demo` opens a working Home screen in the terminal
and exits cleanly. Does **not** talk to Alchemy yet: the runtime consumes
a hardcoded `HomeViewModel` so it can be built and reviewed in isolation.
Real data arrives in `plan/13-alchemy-adapter.md` (phase 2).

## 1. Purpose and scope

- Introduce the `Screen` trait, a `Command` enum and a `ScreenStack`
  container under `src/adapters/ui/`.
- Wrap the existing `home::render` pure function in a stateful
  `HomeScreen` struct that implements `Screen`.
- Add a real Tokio + crossterm event loop in `src/infra/` and expose it
  through a `--demo` CLI switch.
- Keep every existing test green. Add functional tests for the new types.

Out of scope:

- Any network call or Alchemy plumbing.
- Multiple screens (only Home exists today).
- Configuration file parsing.

## 2. Types introduced

### 2.1 `Command`

```rust
pub enum Command {
    None,
    Pop,
    Quit,
    Refresh,
}
```

Returned by every input handler. Resolved by the dispatcher after the
handler returns.

### 2.2 `Screen` trait

```rust
pub trait Screen: Send {
    fn title(&self) -> &str;
    fn render(&self, frame: &mut Frame<'_>, area: Rect);
    fn handle_key(&mut self, key: KeyEvent) -> Command;
    fn tick(&mut self) -> Command;
}
```

- Sync-only for this phase. Async refresh arrives in phase 2 through a
  separate channel-based mechanism; the trait stays stable.
- `Send` bound so screens can sit behind `Box<dyn Screen>` and be owned
  by a stack that is moved between tasks.

### 2.3 `ScreenStack`

```rust
pub struct ScreenStack { /* private */ }

impl ScreenStack {
    pub fn new() -> Self;
    pub fn push(&mut self, screen: Box<dyn Screen>);
    pub fn pop(&mut self) -> Option<Box<dyn Screen>>;
    pub fn top(&self) -> Option<&dyn Screen>;
    pub fn top_mut(&mut self) -> Option<&mut dyn Screen>;
    pub fn is_empty(&self) -> bool;
    pub fn len(&self) -> usize;
}
```

The dispatcher never mutates the inner `Vec` directly: `Command::Pop`
triggers a single `pop`, `Command::Quit` clears the stack.

### 2.4 `HomeScreen`

Holds a `HomeViewModel` (hardcoded for this phase) and a minimal
keybinding table:

| Key        | Action         |
|------------|----------------|
| `q`        | `Command::Quit`|
| `Esc`      | `Command::Pop` |
| any other  | `Command::None`|

The render method delegates to the existing `home::render` pure
function.

## 3. Runtime model

```mermaid
sequenceDiagram
    participant Kbd as Terminal keyboard
    participant In as Input task
    participant Tk as Ticker
    participant Loop as Event loop
    participant Stk as ScreenStack
    participant Term as Terminal

    Kbd->>In: KeyEvent
    In->>Loop: AppEvent::Key
    Tk->>Loop: AppEvent::Tick
    Loop->>Stk: top_mut().handle_key / tick
    Stk-->>Loop: Command
    Loop->>Term: draw(top.render)
```

- Input task: blocking `crossterm::event::read` running inside
  `tokio::task::spawn_blocking`, pushing `AppEvent::Key` values into an
  `mpsc::UnboundedSender`.
- Ticker task: `tokio::time::interval(250ms)` pushing
  `AppEvent::Tick` into the same channel.
- Event loop task: awaits on the receiver, dispatches to the top screen,
  applies the returned `Command`, then redraws.
- Terminal setup/teardown handled by helper functions
  `enter_alternate_screen` / `leave_alternate_screen` that enable raw
  mode, switch buffer, install a panic hook that restores the terminal.

## 4. CLI

`src/infra/mod.rs::run` inspects `std::env::args`:

- If `--demo` is present: set up the Tokio runtime and enter the event
  loop with a Home screen primed with hardcoded view-model data.
- Otherwise: print a short hint to stderr and exit `Ok(())`, preserving
  the current behaviour.

Hint text (pointing at future plans):

```
blockexplorer-tui: real data adapters are not wired yet.
Launch with `cargo run -- --demo` to see the Home screen rendered with
placeholder data.
See plan/12-screen-runtime.md (runtime) and plan/13-alchemy-adapter.md
(real data, upcoming).
```

## 5. Tests

- `tests/functional/screen_stack.rs`: push, pop, top, is_empty, len.
- `tests/functional/home_screen_keys.rs`: constructs a `HomeScreen`,
  feeds `KeyEvent` for `q` and `Esc`, asserts the returned `Command`.
- Existing tests (home_screen_render, home_session, use cases, e2e) all
  keep passing.

No BDD scenario yet for the runtime itself. The runtime is exercised
manually via `cargo run -- --demo` as documented in section 6. When a
full-blown integration harness arrives it will drive the same event
loop.

## 6. Acceptance

- `cargo test` passes (previous tests plus the two new files).
- `cargo clippy --all-targets -- -D warnings` is clean.
- `cargo run -- --demo` opens a terminal screen with the Home layout,
  shows Ethereum placeholder numbers, and exits cleanly when `q` is
  pressed. Pressing `Esc` with only one screen on the stack also exits
  (stack emptied).
- `cargo run` without the flag prints the hint and exits 0.

## 7. Follow-up (not this phase)

- Phase 2 (`plan/13-alchemy-adapter.md`): Alchemy HTTP adapter
  implementing `NetworkStatusPort` and `GasOraclePort`; `HomeScreen`
  holds a `HomeSession` and receives `AppEvent::Data(HomeViewModel)`
  updates from a background refresher task.
- Phase 3 (`plan/14-config-and-credentials.md`): config file parser,
  `ALCHEMY_API_KEY` env var handling, Settings screen MVP; `--demo`
  becomes the fallback used only when no credentials are present.
