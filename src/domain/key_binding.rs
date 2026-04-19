//! Pure domain types for keyboard bindings.
//!
//! Today the TUI hard-codes its bindings inside each screen's
//! `handle_key`. A future Settings slice will allow users to
//! override those bindings from `config.toml`; this module ships the
//! data model and conflict-detection helper so that slice only has
//! to wire the new config section and render the modal.
//!
//! See `plan/10-settings.md` section 12.4.

use std::{collections::HashMap, fmt};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Identifier of the screen a binding applies to. Bindings may be
/// scoped per-screen (`Home::Char('/')` opens the search screen) or
/// global (`ScreenId::Global::Char('q')` quits the app).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenId {
    Global,
    Home,
    Search,
    BlockDetail,
    TxDetail,
    AddressDetail,
    ContractDetail,
    TokenDetail,
    GasTracker,
    Mempool,
    Settings,
}

impl ScreenId {
    /// Human-readable label shown in the conflict modal.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ScreenId::Global => "global",
            ScreenId::Home => "Home",
            ScreenId::Search => "Search",
            ScreenId::BlockDetail => "Block Detail",
            ScreenId::TxDetail => "Tx Detail",
            ScreenId::AddressDetail => "Address Detail",
            ScreenId::ContractDetail => "Contract Detail",
            ScreenId::TokenDetail => "Token Detail",
            ScreenId::GasTracker => "Gas Tracker",
            ScreenId::Mempool => "Mempool",
            ScreenId::Settings => "Settings",
        }
    }
}

/// Semantic action a binding dispatches to. The MVP covers the
/// handlers that actually exist in the current UI; the list grows as
/// new bindings are introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Quit,
    Back,
    OpenSearch,
    OpenMempool,
    OpenGasTracker,
    OpenSettings,
    RefreshNow,
    CopyAddress,
    CopyHash,
}

impl Action {
    /// Human-readable name used in the conflict modal.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Action::Quit => "Quit",
            Action::Back => "Back",
            Action::OpenSearch => "Open Search",
            Action::OpenMempool => "Open Mempool",
            Action::OpenGasTracker => "Open Gas Tracker",
            Action::OpenSettings => "Open Settings",
            Action::RefreshNow => "Refresh Now",
            Action::CopyAddress => "Copy Address",
            Action::CopyHash => "Copy Hash",
        }
    }
}

/// A single `(screen, key) -> action` mapping. Stored as the unit of
/// user-supplied config so errors can point at the exact offending
/// line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub screen: ScreenId,
    pub key: KeyEvent,
    pub action: Action,
}

/// Set of keyboard bindings indexed by `(screen, key)`. Constructed
/// via [`KeyMap::from_entries`] which validates for conflicts up
/// front.
#[derive(Debug, Clone, Default)]
pub struct KeyMap {
    entries: HashMap<(ScreenId, KeyEvent), Action>,
}

impl KeyMap {
    /// Lookup the action for `(screen, key)`, falling back to a
    /// global binding when no screen-specific match exists.
    #[must_use]
    pub fn resolve(&self, screen: ScreenId, key: KeyEvent) -> Option<Action> {
        self.entries
            .get(&(screen, key))
            .copied()
            .or_else(|| self.entries.get(&(ScreenId::Global, key)).copied())
    }

    /// Number of entries in the map.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Construct a `KeyMap` from a list of bindings, reporting every
    /// conflict found along the way. A conflict is any
    /// `(screen, key)` pair that appears twice with different actions.
    /// Identical duplicate entries are silently de-duplicated so
    /// users may safely re-declare a default binding.
    pub fn from_entries(entries: &[KeyBinding]) -> Result<Self, KeyBindConflictReport> {
        let mut map: HashMap<(ScreenId, KeyEvent), Vec<Action>> = HashMap::new();
        for binding in entries {
            let bucket = map.entry((binding.screen, binding.key)).or_default();
            if !bucket.contains(&binding.action) {
                bucket.push(binding.action);
            }
        }

        let mut conflicts = Vec::new();
        let mut entries_map = HashMap::new();
        // Iterate in insertion-stable order by walking the input slice
        // again. This keeps the report deterministic for tests.
        for binding in entries {
            let key = (binding.screen, binding.key);
            if entries_map.contains_key(&key) {
                continue;
            }
            let Some(actions) = map.get(&key) else {
                continue;
            };
            if actions.len() > 1 {
                if !conflicts
                    .iter()
                    .any(|c: &KeyBindConflict| c.screen == binding.screen && c.key == binding.key)
                {
                    conflicts.push(KeyBindConflict {
                        screen: binding.screen,
                        key: binding.key,
                        actions: actions.clone(),
                    });
                }
            } else {
                entries_map.insert(key, actions[0]);
            }
        }

        if conflicts.is_empty() {
            Ok(KeyMap {
                entries: entries_map,
            })
        } else {
            Err(KeyBindConflictReport { conflicts })
        }
    }

    /// Default keyboard map mirroring the handlers currently wired on
    /// `HomeScreen`. Kept in-sync with `src/adapters/ui/home.rs`.
    #[must_use]
    pub fn builtin() -> Self {
        let entries = builtin_bindings();
        KeyMap::from_entries(&entries).expect("builtin bindings must not conflict")
    }
}

/// One conflicting entry: a `(screen, key)` pair with two or more
/// actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBindConflict {
    pub screen: ScreenId,
    pub key: KeyEvent,
    pub actions: Vec<Action>,
}

/// Report surfaced by [`KeyMap::from_entries`] when one or more
/// conflicts are detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBindConflictReport {
    pub conflicts: Vec<KeyBindConflict>,
}

impl fmt::Display for KeyBindConflictReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} keybind conflict(s):", self.conflicts.len())?;
        for c in &self.conflicts {
            write!(f, "  - {} / {}: ", c.screen.label(), format_key(c.key))?;
            for (i, action) in c.actions.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", action.label())?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

impl std::error::Error for KeyBindConflictReport {}

/// Best-effort key formatter for the conflict modal. Intentionally
/// lightweight — if new keys need rendering we extend this match
/// rather than pulling in a formatting crate.
#[must_use]
pub fn format_key(event: KeyEvent) -> String {
    let mut out = String::new();
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        out.push_str("Ctrl+");
    }
    if event.modifiers.contains(KeyModifiers::ALT) {
        out.push_str("Alt+");
    }
    if event.modifiers.contains(KeyModifiers::SHIFT) {
        out.push_str("Shift+");
    }
    match event.code {
        KeyCode::Char(c) => out.push(c),
        KeyCode::Esc => out.push_str("Esc"),
        KeyCode::Enter => out.push_str("Enter"),
        KeyCode::Tab => out.push_str("Tab"),
        KeyCode::Backspace => out.push_str("Backspace"),
        KeyCode::Left => out.push_str("Left"),
        KeyCode::Right => out.push_str("Right"),
        KeyCode::Up => out.push_str("Up"),
        KeyCode::Down => out.push_str("Down"),
        KeyCode::F(n) => {
            use std::fmt::Write as _;
            let _ = write!(out, "F{n}");
        }
        other => {
            use std::fmt::Write as _;
            let _ = write!(out, "{other:?}");
        }
    }
    out
}

fn builtin_bindings() -> Vec<KeyBinding> {
    let none = KeyModifiers::NONE;
    vec![
        KeyBinding {
            screen: ScreenId::Home,
            key: KeyEvent::new(KeyCode::Char('q'), none),
            action: Action::Quit,
        },
        KeyBinding {
            screen: ScreenId::Home,
            key: KeyEvent::new(KeyCode::Esc, none),
            action: Action::Back,
        },
        KeyBinding {
            screen: ScreenId::Home,
            key: KeyEvent::new(KeyCode::Char('/'), none),
            action: Action::OpenSearch,
        },
        KeyBinding {
            screen: ScreenId::Home,
            key: KeyEvent::new(KeyCode::Char('m'), none),
            action: Action::OpenMempool,
        },
        KeyBinding {
            screen: ScreenId::Home,
            key: KeyEvent::new(KeyCode::Char('g'), none),
            action: Action::OpenGasTracker,
        },
        KeyBinding {
            screen: ScreenId::Home,
            key: KeyEvent::new(KeyCode::Char('s'), none),
            action: Action::OpenSettings,
        },
    ]
}
