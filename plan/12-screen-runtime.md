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
    Push(Box<dyn Screen>),
    Replace(Box<dyn Screen>),
    Switch(Box<dyn Screen>),
    OpenModal(Box<dyn Screen>),
    CloseModal,
}
```

Returned by every input handler. Resolved by the dispatcher after the
handler returns. `Switch` clears the stack and pushes its argument;
`OpenModal` sets the modal slot without touching the back stack;
`CloseModal` clears it.

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

A `Command::Pop` that would leave the stack empty is treated as a no-op
at the dispatcher level: Home (or whatever screen happens to be the
root) stays on top and the event loop keeps running. `Command::Quit`
(bound to `q` and SIGINT / Ctrl+C) is the only path out of the loop.
See §7.1 for the shipped semantics.

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
  pressed. Pressing `Esc` on Home is a no-op (the root screen is never
  popped off); see §7.1.
- `cargo run` without the flag prints the hint and exits 0.

## 7.1 Shipped — Root screen protection (slice 1 of "unified detail + global UX", April 2026)

Landed on branch `slice1/home-always-root`:

- **`Command::Pop` on a single-screen stack is a no-op.**
  `ScreenStack::apply_command(Command::Pop)` used to pop the root screen
  and return `Transition::Exit`, so `Esc` on Home accidentally closed
  the app. The dispatcher now returns `Transition::Continue` without
  mutating the stack when `self.screens.len() <= 1` (and no modal is
  open — the modal branch still wins over the pop when a modal is
  visible). Every per-screen `Esc -> Pop` binding keeps working
  unchanged; the guard lives in the stack, not in the screens.
- **`Command::Replace` on an empty stack is a no-op.** Defensive edge
  case symmetric to the above: with an empty stack there is nothing to
  pop first, and the composition root is the only legitimate source of
  the initial screen. Dispatching `Replace` on an empty stack now
  short-circuits to `Transition::Continue` without pushing anything.
- **`Command::Quit` is the only path out of the event loop.** Bound to
  `q` at the screen level and to SIGINT (Ctrl+C) through
  `signal_to_command` in `src/infra/runtime.rs`. Tests:
  `tests/functional/screen_stack.rs::pop_on_single_screen_stack_is_noop`,
  `pop_on_empty_stack_is_noop`,
  `pop_with_two_screens_shrinks_to_one`;
  `tests/e2e/features/home.feature` scenarios
  `Esc on Home keeps the app running` and
  `q on Home still quits the app`.

## 7. Shipped follow-ups (April 2026, branch `probe/8.13-screen-runtime`)

Landed under `plan/15-backlog.md §8.13`:

- **Panic hook installed before `enter_tui`.** `run_event_loop` now calls
  `install_panic_hook()` as the very first step, so a panic while setting up
  raw mode or the alternate screen still restores the terminal. Teardown is
  guarded by an `AtomicBool` flag so `leave_tui` and the panic hook may both
  fire safely without double-disabling raw mode.
- **Ctrl+C handled via `tokio::signal::ctrl_c()`.** The dispatcher now
  `tokio::select!`s between the event channel and the signal future; a
  SIGINT is mapped to `Command::Quit`, taking the same teardown path as the
  `q` binding.
- **`Command` enum grown** with `OpenModal(Box<dyn Screen>)`, `CloseModal`
  and `Switch(Box<dyn Screen>)`. `Push(_)` / `Replace(_)` already shipped
  with §11. `Switch` clears the stack and pushes the target screen — the
  dispatcher uses it for global jumps (`gh`, `gs`, `gm`) so the back stack
  never drags context across top-level views.
- **Modal slot on `ScreenStack`.** Modals live in a dedicated
  `modal: Option<Box<dyn Screen>>` slot, rendered on top of the stack top
  without participating in the back stack. Key events flow to the modal
  first; `Command::Pop` from the modal closes it and yields control back
  to the underlying screen. `ScreenStack::apply_command` centralises the
  transition logic shared by the runtime dispatcher and the BDD harness.
- **`HelpModal` (bound to `?`).** A lightweight modal rendered by the
  dispatcher when the user presses `?`. Content comes from the active
  screen's `KeyBindHints` (see `plan/10-settings.md §12.4`) when present
  and falls back to the global map otherwise.
- **Global search (`/`) via a `GlobalKeyMap`.** `run_event_loop` takes a
  `GlobalKeyMap` with optional factories for `/` (search) and `?` (help).
  When the current screen does not claim the key, the dispatcher opens
  the configured factory as a modal. `HomeScreen::with_search_factory`
  still works for tests that construct the stack directly; `infra` routes
  the same factory through the global map so every screen picks it up.

## 8. Still deferred

- Migration of the Gas Tracker unit converter (`src/adapters/ui/gas_tracker.rs`)
  from its local `ConverterState` to a `Command::OpenModal` dispatch.
  Shape is understood (return `Command::OpenModal(Box::new(ConverterModal))`
  on `u`, let the dispatcher render/tick it and route
  `Command::CloseModal` on `Esc`) but the screen currently owns cross-state
  with its rolling histogram so the refactor is left for a dedicated probe.
- `ConfirmModal` and `InputModal`: the `HelpModal` lands first because it
  is the only modal the current screens need; the `Confirm`/`Input`
  counterparts are deferred until the first use case wants them (copy-on-
  confirm or runtime edit from Settings, see `plan/10-settings.md §11.2`).
- Reconnecting-badge reuse via the new modal system for the mempool
  screen (`plan/5-mempool.md §11.3.4`). The modal system is ready; the
  badge is still a local overlay inside `MempoolScreen`.

## 9. Follow-up (not this phase)

- Phase 2 (`plan/13-alchemy-adapter.md`): Alchemy HTTP adapter
  implementing `NetworkStatusPort` and `GasOraclePort`; `HomeScreen`
  holds a `HomeSession` and receives `AppEvent::Data(HomeViewModel)`
  updates from a background refresher task.
- Phase 3 (`plan/14-config-and-credentials.md`): config file parser,
  `ALCHEMY_API_KEY` env var handling, Settings screen MVP; `--demo`
  becomes the fallback used only when no credentials are present.
