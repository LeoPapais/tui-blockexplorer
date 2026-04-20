# 17 — Navigable values, real clipboard, universal field cursor

> Status: accepted (slice 4 of "unified detail screen + global UX").
>
> Prior art: [plan/1-home.md], [plan/3-block-detail.md] §12.1,
> [plan/4-tx-detail.md] §12.6.2, [plan/6-address-detail.md] §9,
> [plan/8-token-detail.md], [plan/16-unified-address-detail.md], and the
> backlog entries [plan/15-backlog.md] §8.4, §8.5, §8.7, §8.13, §8.16.

## 1. Motivation

Before this slice every detail screen reimplemented its own clipboard sink
as an `Option<String>` that only existed to satisfy tests. The `y` / `Y`
bindings copied fixed values chosen per tab, never the "thing the user is
looking at". `arboard` was already a declared dependency but no production
code reached into it, so the explorer ran with a no-op clipboard.

Cross-screen navigation was equally uneven: list rows (Address
Transactions, Block Tx list) responded to Enter, but Overview fields such
as `from`, `to`, `block_number` were inert — the user could not jump from
a tx's `to` address to its Address Detail screen.

This slice introduces a single cursor primitive and wires it to a real
`ClipboardPort` and a `NavigationFactory`, so every detail screen exposes
the same mental model: arrow keys walk a list of values, `y` copies, and
`Enter` jumps.

## 2. Domain

A new value object under `src/domain/navigable.rs`:

```rust
pub enum NavigableValue {
    Address(Address),
    TxHash(TxHash),
    BlockNumber(BlockNumber),
    BlockHash(BlockHash),
    TokenAddress(Address),
    EnsName(String),
    Plain(String),
}
```

* `copy_text(&self) -> String` returns the canonical textual form
  (hex for addresses and hashes, decimal for block numbers, raw string
  for ENS names / plain values).
* `can_navigate(&self) -> bool` is `true` for every variant except
  `Plain`, which is copy-only.

The enum is re-exported at `src/domain/mod.rs` next to the rest of the
value objects.

## 3. Application port

`src/application/ports/clipboard.rs` introduces the outbound
`ClipboardPort`:

```rust
pub trait ClipboardPort: Send + Sync {
    fn set(&self, text: &str) -> Result<(), DomainError>;
}
```

No getter — the TUI never reads the clipboard. Implementations are
allowed to degrade to a no-op as long as they keep the `Ok(())`
contract (see §5).

## 4. UI widget: `FieldCursor`

`src/adapters/ui/field_cursor.rs` owns the cursor state machine and the
`NavigationFactory` trait:

```rust
pub struct FieldEntry {
    pub value: NavigableValue,
    pub label: &'static str,
}

pub enum CursorDir { Left, Right, Up, Down }

pub struct FieldCursor {
    active: Option<usize>,
}

impl FieldCursor {
    pub fn new() -> Self;
    pub fn is_active(&self) -> bool;
    pub fn active(&self) -> Option<usize>;
    pub fn deactivate(&mut self);
    pub fn move_in(&mut self, len: usize, dir: CursorDir);
    pub fn current<'a>(&self, fields: &'a [FieldEntry]) -> Option<&'a FieldEntry>;
}

pub trait NavigationFactory: Send + Sync {
    fn open(
        &self,
        value: &NavigableValue,
        active_chain: Chain,
    ) -> Option<Box<dyn Screen>>;
}
```

The cursor is deliberately a 1-D index into a reading-order `Vec<FieldEntry>`:

* `Left` / `Up` step back (saturating at 0);
* `Right` / `Down` step forward (clamped at `len - 1`);
* when inactive, any direction activates at index 0.

Visual layout nuances (multi-column forms, etc.) are honoured by the
screen populating `fields()` in reading order.

## 5. Adapters

### 5.1 Production clipboard (`src/adapters/clipboard/arboard.rs`)

`ArboardClipboard` wraps `arboard::Clipboard` behind a `Mutex` so the
handle can be shared between tasks. The constructor
(`ArboardClipboard::new()`) returns an `ArboardClipboard` even when
`arboard::Clipboard::new()` fails, logging a single warning and
degrading to a no-op implementation.

Rationale: Debian/CI boxes without a DISPLAY or Wayland socket are
expected to crash inside `arboard::Clipboard::new()`. The adapter MUST
NOT panic — failing to copy is recoverable, the TUI keeps running.

The constructor accepts an injected factory closure
(`ArboardClipboard::with_factory`) so a unit test can simulate the
headless failure path without spinning up a display.

### 5.2 Test stub (`tests/support/stubs.rs::StubClipboard`)

`StubClipboard` wraps `Mutex<Option<String>>`. `set` stores the value;
`last_copied()` exposes it to assertions. Every per-screen cursor test
drives this stub.

### 5.3 Navigation stubs / live wiring

`tests/support/stubs.rs::StubNavigationFactory` records every
`NavigableValue` the screen asked to navigate to; `recorded()` returns a
`Vec<NavigableValue>` for assertions.

`src/infra/navigate.rs::LiveNavigationFactory` maps
`NavigableValue` → `Box<dyn Screen>` using the same `live_*_screen`
helpers the search router already calls:

| Variant                | Target screen / tab                         |
|------------------------|---------------------------------------------|
| `Address(a)`           | `live_address_detail_screen(..., Overview)` |
| `EnsName(n)`           | `live_address_detail_screen` once resolved  |
| `TokenAddress(a)`      | `live_address_detail_screen(..., Token)`    |
| `TxHash(h)`            | `live_tx_detail_screen(chain, h)`           |
| `BlockNumber(n)`       | `live_block_detail_screen(..., Number(n))`  |
| `BlockHash(h)`         | `live_block_detail_screen(..., Hash(h))`    |
| `Plain(_)`             | `None` (copy-only)                          |

ENS forward-resolution stays in the search pipeline today — the MVP
`LiveNavigationFactory` opens `AddressDetail` keyed on the hex already
held by the screen. Forward-ENS routing from the cursor is tracked as a
follow-up under `plan/15-backlog.md` §3.1.

## 6. Per-screen matrix

Scope: every screen ships at least one cursor-navigable field. Lists
retain their pre-existing Enter contract (the cursor is treated as "off"
inside list tabs) so the slice is strictly additive.

| Screen            | Cursor scope                   | Shipped fields                                           |
|-------------------|--------------------------------|----------------------------------------------------------|
| Home              | Always                         | latest block number, chain name (Plain), gas tiers (Plain)|
| AddressDetail     | Overview tab                   | address, ENS (when present)                              |
| BlockDetail       | Overview tab                   | block number, block hash, parent hash, miner             |
| TxDetail          | Overview tab                   | tx hash, from address, to address, block number         |
| Settings          | Always                         | config path + active chain as `Plain`                    |
| SearchScreen      | Inherits result navigation      | no change (Enter already navigates)                     |

Full field coverage across every sub-tab of AddressDetail (contract
source, logs table, storage slots, etc.) remains tracked under
`plan/15-backlog.md` §8.16 as an incremental follow-up — the cursor
abstraction is in place and only needs additional `FieldEntry` builders
per sub-tab.

## 7. Input rules

On every screen that opts in:

1. `Esc` — if the cursor is active, deactivate and return `Command::None`
   without popping the screen; otherwise fall through to the
   pre-existing handler.
2. Arrow keys — the cursor activates on the first press (at index 0 of
   the current `fields()`). Subsequent presses move within the list.
3. `y` while the cursor is active — call `ClipboardPort::set` with
   `current().copy_text()`. When the cursor is inactive `y` keeps its
   pre-existing tab-specific behaviour.
4. `Enter` while the cursor is active — if
   `current().can_navigate()` and a `NavigationFactory` is wired, return
   `Command::Push(factory.open(value, chain))`. Otherwise
   `Command::None`.
5. `q` always quits (unchanged).

## 8. Tests

### 8.1 Functional (`tests/functional/`)

* `field_cursor.rs` — standalone state-machine tests
  (`cursor_starts_inactive`, `cursor_activates_on_first_arrow_key`,
  `right_arrow_moves_to_next_field_in_reading_order`,
  `down_arrow_advances`, `wrap_around_at_edges_is_a_noop`,
  `esc_deactivates_cursor_without_popping_screen`).
* `arboard_clipboard.rs` — one `#[ignore]` test documenting the real
  arboard path and one graceful-degradation test that injects a failing
  factory.
* Per-screen cursor tests: `home_cursor.rs`, `address_detail_cursor.rs`,
  `tx_detail_cursor.rs`, `block_detail_cursor.rs`, `settings_cursor.rs`. Each wires a
  `StubClipboard` + `StubNavigationFactory`, drives the expected arrow
  sequence, and asserts on the recorded `last_copied()` /
  `recorded()` calls via `pretty_assertions::assert_eq!`.

### 8.2 BDD (`tests/e2e/features/*.feature`)

BDD coverage for the cursor is deferred to a follow-up slice. The
per-screen functional tests (§8.1) already exercise the cursor
through the same `StubClipboard` + `StubNavigationFactory` harness
the scenarios would use; growing the Cucumber world with the cursor
stubs is a mechanical follow-up once the primitive has settled in
production.

Tracked under `plan/15-backlog.md` §8.16 alongside the rest of the
per-sub-tab cursor coverage.

## 9. Deferred

* Full coverage of the nested AddressDetail sub-tabs (Contract
  source/ABI/Events/Storage, Token Transfers / Chart). The primitive
  lands now; each sub-tab earns its fields incrementally under the
  existing `plan/6`, `plan/7`, `plan/8` sections. Tracked in
  `plan/15-backlog.md` §8.16.
* Forward-ENS routing from a `NavigableValue::EnsName` hit (needs
  `EnsResolverPort` on the cursor path). Tracked under
  `plan/15-backlog.md` §3.1.
* Mouse click-to-activate the cursor. Out of scope — mouse events are
  not wired into the runtime today.
