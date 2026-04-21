//! Transaction Detail screen.
//!
//! Surfaces Overview / Logs / Asset Changes / State Changes / Raw
//! tabs. See `plan/4-tx-detail.md` sections 12.3 (MVP), 12.4
//! (expansion) and 13 (follow-up fixes).
//!
//! Key layout:
//! - `Tab` / `Shift+Tab` cycle main tabs from the body. When focus is
//!   on the tab strip (`↑` from the body), `←` / `→` move main tabs
//!   and `↓` returns to the body.
//! - On the Overview tab, `Up` / `Down` move a row-level selection
//!   cursor and `y` copies the selected row's canonical value.
//!   Humanized amounts gain an inline muted "raw" suffix while the
//!   row is selected (see plan 13.2).
//! - On the Logs tab, `Up` / `Down` move the selected log in the
//!   left pane; `Enter` / `Right` focus the field list in the
//!   right pane, where `Up` / `Down` walk fields and `y` copies
//!   the selected field value. `Esc` or `Left` returns focus to
//!   the log list (while in that pane, `Left` switches tabs as
//!   usual).
//! - `Raw`, `Asset Changes` and `State Changes` remain scrollable
//!   via `PageUp` / `PageDown` / `Home` / `End`, now bounded by
//!   `ScrollState` (see plan 13.3).

use std::borrow::Cow;
use std::cell::Cell;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block as RatBlock, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::{
        detail_focus::{
            DetailFocusLayer, DetailTabStrip, detail_body_border_style, detail_header_border_style,
            tab_strip_border_style, tab_strip_highlight_style,
        },
        field_cursor::CursorServices,
        format::{humanize_eth, humanize_gas_units, humanize_gwei, humanize_token_units},
        screen::{Command, Screen},
        scroll::ScrollState,
        theme::PalettePreset,
    },
    application::{DecodedLog, DecodedMethod, EventAbi, LoadStatus, SignatureSource, TxView},
    domain::{
        Address, AddressStateDiff, AssetChange, AssetChangeKind, AssetKind, CallNode, Chain,
        DiffChange, LogEntry, NavigableValue, StateDiff, TxHash, TxStatus, Wei,
    },
};

/// Nested keyboard focus within the transaction Logs tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxLogsPane {
    List,
    Decoded,
    Raw,
}

pub struct TxFeed {
    pub input_tx: UnboundedSender<TxHash>,
    pub updates_rx: UnboundedReceiver<TxView>,
}

pub struct TxFeedSender {
    pub updates_tx: UnboundedSender<TxView>,
    pub input_rx: UnboundedReceiver<TxHash>,
}

#[must_use]
pub fn tx_feed() -> (TxFeed, TxFeedSender) {
    let (input_tx, input_rx) = unbounded_channel();
    let (updates_tx, updates_rx) = unbounded_channel();
    (
        TxFeed {
            input_tx,
            updates_rx,
        },
        TxFeedSender {
            updates_tx,
            input_rx,
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxTab {
    Overview,
    Logs,
    Internal,
    AssetChanges,
    StateChanges,
    Raw,
}

impl TxTab {
    const ALL: [TxTab; 6] = [
        TxTab::Overview,
        TxTab::Logs,
        TxTab::Internal,
        TxTab::AssetChanges,
        TxTab::StateChanges,
        TxTab::Raw,
    ];

    fn next(self) -> Self {
        let idx = self.index();
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    fn previous(self) -> Self {
        let idx = self.index();
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    fn index(self) -> usize {
        match self {
            TxTab::Overview => 0,
            TxTab::Logs => 1,
            TxTab::Internal => 2,
            TxTab::AssetChanges => 3,
            TxTab::StateChanges => 4,
            TxTab::Raw => 5,
        }
    }

    fn label(self) -> &'static str {
        match self {
            TxTab::Overview => "Overview",
            TxTab::Logs => "Logs",
            TxTab::Internal => "Internal",
            TxTab::AssetChanges => "Asset Changes",
            TxTab::StateChanges => "State Changes",
            TxTab::Raw => "Raw",
        }
    }
}

/// Which pane of the Logs tab currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogsFocus {
    List,
    Detail,
    Raw,
}

/// A selectable row on the Overview tab. Holds a humanized display
/// value, a canonical `copy_value`, and optionally a muted "raw"
/// suffix appended when the row is selected (plan 13.1 + 13.2).
#[derive(Debug, Clone, PartialEq, Eq)]
struct OverviewRow {
    label: &'static str,
    display: String,
    copy_value: String,
    raw_hint: Option<String>,
}

/// A decoded field inside a log's detail pane. Mirrors plan 13.4.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LogField {
    label: String,
    display: String,
    copy_value: String,
    raw_hint: Option<String>,
}

pub struct TxDetailScreen {
    #[allow(dead_code)]
    chain: Chain,
    /// Hash originally requested. Kept around so `s` can re-send the
    /// same request on the feed when the user asks to re-simulate a
    /// pending tx (plan 12.6.3).
    hash: TxHash,
    current: Option<TxView>,
    feed: TxFeed,
    active_tab: TxTab,
    focus_layer: DetailFocusLayer,
    /// Row-level cursor for the Overview tab (plan 13.1).
    overview_row: usize,
    /// Left-pane log cursor for the Logs tab (plan 13.4).
    logs_selected: usize,
    /// Right-pane field cursor for the Logs tab (plan 13.4).
    logs_field: usize,
    /// Line cursor on the Raw JSON tab (mirrors Overview rows).
    raw_row: usize,
    /// Pane focus inside the Logs tab (plan 13.4).
    logs_focus: LogsFocus,
    /// Selected line inside the Raw log dump (copy + highlight).
    logs_raw_line: usize,
    /// Bounded scroll used by tabs whose content is rendered as a
    /// single `Paragraph` (plan 13.3). Wrapped in a `Cell` so the
    /// render path can refresh `content_height` / `viewport_height`
    /// through `&self` and `handle_key` can then clamp the offset
    /// against up-to-date dimensions.
    scroll: Cell<ScrollState>,
    /// Scroll for the Decoded fields pane on the Logs tab.
    logs_decoded_scroll: Cell<ScrollState>,
    /// Scroll for the Raw log pane on the Logs tab.
    logs_raw_scroll: Cell<ScrollState>,
    /// Last value produced by the `y` "copy" binding. The real
    /// runtime wires a clipboard adapter on top; tests inspect the
    /// field directly.
    last_copied_value: Option<String>,
    /// Count of `s` re-simulate hits delivered to the feed. Tests
    /// inspect it; the UI itself never surfaces the value. See
    /// `plan/4-tx-detail.md` section 12.6.3.
    resimulate_count: u32,
    /// Cursor collaborators (real clipboard + navigation factory).
    /// When present, `y` copies via `ClipboardPort::set` and `Enter`
    /// pushes the next screen through `NavigationFactory::open`. See
    /// `plan/17-navigable-values.md` §4.
    cursor_services: Option<CursorServices>,
}

impl TxDetailScreen {
    #[must_use]
    pub fn loading(chain: Chain, hash: TxHash, feed: TxFeed) -> Self {
        let _ = feed.input_tx.send(hash);
        Self {
            chain,
            hash,
            current: None,
            feed,
            active_tab: TxTab::Overview,
            focus_layer: DetailFocusLayer::Content,
            overview_row: 0,
            logs_selected: 0,
            logs_field: 0,
            raw_row: 0,
            logs_focus: LogsFocus::List,
            logs_raw_line: 0,
            scroll: Cell::new(ScrollState::new()),
            logs_decoded_scroll: Cell::new(ScrollState::new()),
            logs_raw_scroll: Cell::new(ScrollState::new()),
            last_copied_value: None,
            resimulate_count: 0,
            cursor_services: None,
        }
    }

    /// Wire cursor-owned clipboard + navigation. See
    /// `plan/17-navigable-values.md` §4.
    #[must_use]
    pub fn with_cursor_services(mut self, services: CursorServices) -> Self {
        self.cursor_services = Some(services);
        self
    }

    /// Navigable value backing the currently-selected Overview row,
    /// when the label maps to an address, block number or tx hash.
    /// Returns `None` for copy-only fields such as gas price or
    /// nonce. See `plan/17-navigable-values.md` §6.
    #[must_use]
    pub fn overview_navigable_value(&self) -> Option<NavigableValue> {
        let view = self.current.as_ref()?;
        let rows = overview_rows(view);
        let row = rows.get(self.overview_row)?;
        match row.label {
            "Hash" => Some(NavigableValue::TxHash(view.tx.hash)),
            "Block" => view.tx.block_number.map(NavigableValue::BlockNumber),
            "From" => Some(NavigableValue::Address(view.tx.from)),
            "To" => view.tx.to.map(NavigableValue::Address),
            _ => None,
        }
    }

    #[must_use]
    pub fn current(&self) -> Option<&TxView> {
        self.current.as_ref()
    }

    #[must_use]
    pub fn active_tab(&self) -> TxTab {
        self.active_tab
    }

    /// Whether keyboard focus is on the tab strip or the body.
    #[must_use]
    pub const fn focus_layer(&self) -> DetailFocusLayer {
        self.focus_layer
    }

    /// Currently selected row on the Overview tab. Returns `0`
    /// when the view has not loaded yet.
    #[must_use]
    pub fn overview_selected_row(&self) -> usize {
        self.overview_row
    }

    /// Label of the currently selected Overview row, if any. Used
    /// by functional tests.
    #[must_use]
    pub fn overview_selected_label(&self) -> Option<&str> {
        self.overview_rows().get(self.overview_row).map(|r| r.label)
    }

    /// Canonical copy value of the currently selected Overview
    /// row, if any. Used by functional tests.
    #[must_use]
    pub fn overview_selected_copy_value(&self) -> Option<String> {
        self.overview_rows()
            .get(self.overview_row)
            .map(|r| r.copy_value.clone())
    }

    /// The last value produced by a `y` keypress. Cleared when the
    /// screen is popped; never cleared on tab change.
    #[must_use]
    pub fn last_copied_value(&self) -> Option<String> {
        self.last_copied_value.clone()
    }

    /// Which nested pane of the Logs tab owns focus (`None` when the
    /// active tab is not Logs).
    #[must_use]
    pub fn logs_tab_pane(&self) -> Option<TxLogsPane> {
        if self.active_tab != TxTab::Logs {
            return None;
        }
        Some(match self.logs_focus {
            LogsFocus::List => TxLogsPane::List,
            LogsFocus::Detail => TxLogsPane::Decoded,
            LogsFocus::Raw => TxLogsPane::Raw,
        })
    }

    /// Number of successful `s` re-simulate keystrokes handled since
    /// construction. Mined txs ignore `s`; pending txs increment the
    /// counter and re-send the tx hash on the feed.
    ///
    /// See `plan/4-tx-detail.md` section 12.6.3.
    #[must_use]
    pub fn resimulate_count(&self) -> u32 {
        self.resimulate_count
    }

    /// Vertical scroll offset for the Logs → Decoded pane (tests only).
    #[cfg(test)]
    #[must_use]
    pub fn logs_decoded_scroll_offset_for_test(&self) -> u16 {
        self.logs_decoded_scroll.get().offset()
    }

    /// Vertical scroll offset for the Logs → Raw pane (tests only).
    #[cfg(test)]
    #[must_use]
    pub fn logs_raw_scroll_offset_for_test(&self) -> u16 {
        self.logs_raw_scroll.get().offset()
    }

    fn drain_feed(&mut self) {
        while let Ok(view) = self.feed.updates_rx.try_recv() {
            self.current = Some(view);
            self.focus_layer = DetailFocusLayer::Content;
            self.overview_row = 0;
            self.logs_selected = 0;
            self.logs_field = 0;
            self.raw_row = 0;
            self.logs_focus = LogsFocus::List;
            self.logs_raw_line = 0;
            self.with_scroll(|s| s.reset());
            self.with_logs_decoded_scroll(|s| s.reset());
            self.with_logs_raw_scroll(|s| s.reset());
        }
    }

    fn with_logs_decoded_scroll(&self, f: impl FnOnce(&mut ScrollState)) {
        let mut s = self.logs_decoded_scroll.get();
        f(&mut s);
        self.logs_decoded_scroll.set(s);
    }

    fn with_logs_raw_scroll(&self, f: impl FnOnce(&mut ScrollState)) {
        let mut s = self.logs_raw_scroll.get();
        f(&mut s);
        self.logs_raw_scroll.set(s);
    }

    /// Helper to mutate the inner `ScrollState` through the `Cell`.
    fn with_scroll(&self, f: impl FnOnce(&mut ScrollState)) {
        let mut s = self.scroll.get();
        f(&mut s);
        self.scroll.set(s);
    }

    fn overview_rows(&self) -> Vec<OverviewRow> {
        match self.current.as_ref() {
            Some(view) => overview_rows(view),
            None => Vec::new(),
        }
    }

    fn log_fields(&self, log: &DecodedLog) -> Vec<LogField> {
        log_fields(log)
    }
}

impl Screen for TxDetailScreen {
    fn title(&self) -> &str {
        "Transaction"
    }

    fn breadcrumb_label(&self) -> Cow<'_, str> {
        Cow::Owned(format!("Tx {}", short_hex(&self.hash.to_hex())))
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(3),
            ])
            .split(area);

        let palette = PalettePreset::DarkDefault.palette();
        let header_border = detail_header_border_style(self.focus_layer, &palette);

        let header = match self.current.as_ref() {
            Some(view) => format!("Tx {}", short_hex(&view.tx.hash.to_hex())),
            None => "Tx (loading...)".to_string(),
        };
        frame.render_widget(
            Paragraph::new(header).block(
                RatBlock::default()
                    .borders(Borders::ALL)
                    .border_style(header_border)
                    .title("Transaction"),
            ),
            chunks[0],
        );

        let titles: Vec<Line<'static>> = TxTab::ALL
            .iter()
            .map(|t| Line::from(format!(" {} ", t.label())))
            .collect();
        let tab_border =
            tab_strip_border_style(self.focus_layer, DetailTabStrip::Main, false, &palette);
        let tab_hi =
            tab_strip_highlight_style(self.focus_layer, DetailTabStrip::Main, false, &palette);
        frame.render_widget(
            Tabs::new(titles)
                .select(self.active_tab.index())
                .block(
                    RatBlock::default()
                        .borders(Borders::ALL)
                        .border_style(tab_border)
                        .title("Tabs  —  ←/→ when strip focused · Tab cycles"),
                )
                .divider(" ")
                .highlight_style(tab_hi),
            chunks[1],
        );

        self.render_body(frame, chunks[2]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        // Global.
        match key.code {
            KeyCode::Char('q') => return Command::Quit,
            KeyCode::Esc => {
                if self.active_tab == TxTab::Logs {
                    match self.logs_focus {
                        LogsFocus::Raw => {
                            self.logs_focus = LogsFocus::Detail;
                            return Command::None;
                        }
                        LogsFocus::Detail => {
                            self.logs_focus = LogsFocus::List;
                            return Command::None;
                        }
                        LogsFocus::List => {}
                    }
                }
                return Command::Pop;
            }
            _ => {}
        }

        // Tab navigation (plan 13.5). `Shift+Tab` fires as
        // `KeyCode::BackTab` on most terminals, but some emit
        // `Shift` modifier + `Tab`; handle both.
        let is_back_tab = key.code == KeyCode::BackTab
            || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT));
        if is_back_tab
            && self.focus_layer == DetailFocusLayer::Content
            && self.active_tab == TxTab::Logs
            && self.logs_focus == LogsFocus::Raw
        {
            self.logs_focus = LogsFocus::Detail;
            return Command::None;
        }
        if is_back_tab {
            self.switch_tab(self.active_tab.previous());
            return Command::None;
        }
        if key.code == KeyCode::Tab
            && self.focus_layer == DetailFocusLayer::Content
            && self.active_tab == TxTab::Logs
            && self.logs_focus == LogsFocus::Detail
        {
            self.logs_focus = LogsFocus::Raw;
            self.logs_raw_line = 0;
            self.with_logs_raw_scroll(|s| s.reset());
            return Command::None;
        }
        if key.code == KeyCode::Tab {
            self.switch_tab(self.active_tab.next());
            return Command::None;
        }

        if self.focus_layer == DetailFocusLayer::MainTabs {
            match key.code {
                KeyCode::Left => {
                    self.switch_tab(self.active_tab.previous());
                    return Command::None;
                }
                KeyCode::Right => {
                    self.switch_tab(self.active_tab.next());
                    return Command::None;
                }
                KeyCode::Down => {
                    self.focus_layer = DetailFocusLayer::Content;
                    return Command::None;
                }
                KeyCode::Up => return Command::None,
                _ => {}
            }
        }

        // `y` copies the selected row / field, regardless of tab.
        if key.code == KeyCode::Char('y') {
            self.copy_selected();
            return Command::None;
        }

        // `s` re-simulates when the tx is still pending (plan 12.6.3).
        // Mined txs have no meaningful re-simulate semantics, so the
        // key is a no-op for them.
        if key.code == KeyCode::Char('s') {
            self.resimulate();
            return Command::None;
        }

        // Enter on the Overview tab opens the matching detail screen
        // (Address / Block / Tx) for the selected row, when the
        // navigation factory is wired. See
        // `plan/17-navigable-values.md` §6.
        if key.code == KeyCode::Enter
            && self.active_tab == TxTab::Overview
            && self.focus_layer == DetailFocusLayer::Content
        {
            if let (Some(value), Some(services)) = (
                self.overview_navigable_value(),
                self.cursor_services.as_ref(),
            ) && let Some(screen) = services.open(&value)
            {
                return Command::Push(screen);
            }
            return Command::None;
        }

        if self.focus_layer == DetailFocusLayer::Content {
            match self.active_tab {
                TxTab::Overview => self.handle_overview_key(key),
                TxTab::Logs => self.handle_logs_key(key),
                TxTab::Raw => self.handle_raw_key(key),
                TxTab::Internal | TxTab::AssetChanges | TxTab::StateChanges => {
                    self.handle_scroll_key(key);
                }
            }
        }
        Command::None
    }

    fn tick(&mut self) -> Command {
        self.drain_feed();
        Command::None
    }

    fn footer_hints(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("Tab", "Tabs"),
            ("←/→", "Tab row"),
            ("↑/↓", "Focus"),
            ("Enter", "Open"),
            ("y", "Copy"),
            ("s", "Re-simulate"),
            ("Esc", "Back"),
        ]
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl TxDetailScreen {
    fn switch_tab(&mut self, target: TxTab) {
        self.active_tab = target;
        self.with_scroll(|s| s.reset());
        if target == TxTab::Logs {
            self.logs_focus = LogsFocus::List;
            self.logs_field = 0;
            self.logs_raw_line = 0;
            self.with_logs_decoded_scroll(|s| s.reset());
            self.with_logs_raw_scroll(|s| s.reset());
        }
        if target == TxTab::Raw {
            self.raw_row = 0;
        }
    }

    fn handle_overview_key(&mut self, key: KeyEvent) {
        let rows = self.overview_rows();
        if rows.is_empty() {
            return;
        }
        match key.code {
            KeyCode::Up => {
                if self.overview_row == 0 {
                    self.focus_layer = DetailFocusLayer::MainTabs;
                } else {
                    self.overview_row -= 1;
                }
            }
            KeyCode::Down if self.overview_row + 1 < rows.len() => {
                self.overview_row += 1;
            }
            KeyCode::Down => {}
            KeyCode::Char('k') => {
                self.overview_row = (self.overview_row + rows.len() - 1) % rows.len();
            }
            KeyCode::Char('j') => {
                self.overview_row = (self.overview_row + 1) % rows.len();
            }
            KeyCode::Home => self.overview_row = 0,
            KeyCode::End => self.overview_row = rows.len() - 1,
            _ => {}
        }
    }

    fn handle_logs_key(&mut self, key: KeyEvent) {
        let Some(view) = self.current.as_ref() else {
            return;
        };
        let log_count = view.decoded_logs.len();
        if log_count == 0 && matches!(key.code, KeyCode::Up | KeyCode::Char('k')) {
            self.focus_layer = DetailFocusLayer::MainTabs;
            return;
        }
        match self.logs_focus {
            LogsFocus::List => match key.code {
                KeyCode::Up | KeyCode::Char('k') if log_count > 0 => {
                    if self.logs_selected == 0 {
                        self.focus_layer = DetailFocusLayer::MainTabs;
                    } else {
                        self.logs_selected -= 1;
                        self.logs_field = 0;
                        self.logs_raw_line = 0;
                        self.with_logs_decoded_scroll(|s| s.reset());
                    }
                }
                KeyCode::Down if log_count > 0 => {
                    if self.logs_selected + 1 < log_count {
                        self.logs_selected += 1;
                    }
                    self.logs_field = 0;
                    self.logs_raw_line = 0;
                    self.with_logs_decoded_scroll(|s| s.reset());
                }
                KeyCode::Char('j') if log_count > 0 => {
                    self.logs_selected = (self.logs_selected + 1) % log_count;
                    self.logs_field = 0;
                    self.logs_raw_line = 0;
                    self.with_logs_decoded_scroll(|s| s.reset());
                }
                KeyCode::Right | KeyCode::Enter if log_count > 0 => {
                    self.logs_focus = LogsFocus::Detail;
                    self.logs_field = 0;
                    self.with_logs_decoded_scroll(|s| s.reset());
                }
                _ => {}
            },
            LogsFocus::Detail => {
                if log_count == 0 {
                    self.logs_focus = LogsFocus::List;
                    return;
                }
                let log = &view.decoded_logs[self.logs_selected];
                let fields = self.log_fields(log);
                if fields.is_empty() {
                    self.logs_focus = LogsFocus::List;
                    return;
                }
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.logs_field = (self.logs_field + fields.len() - 1) % fields.len();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if self.logs_field + 1 < fields.len() {
                            self.logs_field += 1;
                        } else {
                            self.logs_focus = LogsFocus::Raw;
                            self.logs_raw_line = 0;
                            self.with_logs_raw_scroll(|s| s.reset());
                        }
                    }
                    KeyCode::PageUp => {
                        self.logs_field = self.logs_field.saturating_sub(10);
                    }
                    KeyCode::PageDown => {
                        self.logs_field = (self.logs_field + 10).min(fields.len() - 1);
                    }
                    KeyCode::Home => {
                        self.logs_field = 0;
                    }
                    KeyCode::End => {
                        self.logs_field = fields.len() - 1;
                    }
                    KeyCode::Left | KeyCode::Backspace => {
                        self.logs_focus = LogsFocus::List;
                        self.with_logs_decoded_scroll(|s| s.reset());
                    }
                    _ => {}
                }
            }
            LogsFocus::Raw => {
                if log_count == 0 {
                    self.logs_focus = LogsFocus::List;
                    return;
                }
                let log = &view.decoded_logs[self.logs_selected];
                let raw_dump = log_raw_dump(log);
                let line_count = raw_dump.lines().count().max(1);
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if self.logs_raw_line > 0 {
                            self.logs_raw_line -= 1;
                        } else {
                            self.logs_focus = LogsFocus::Detail;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') if self.logs_raw_line + 1 < line_count => {
                        self.logs_raw_line += 1;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {}
                    KeyCode::PageUp => {
                        self.logs_raw_line = self.logs_raw_line.saturating_sub(10);
                    }
                    KeyCode::PageDown => {
                        self.logs_raw_line = (self.logs_raw_line + 10).min(line_count - 1);
                    }
                    KeyCode::Home => {
                        self.logs_raw_line = 0;
                    }
                    KeyCode::End => {
                        self.logs_raw_line = line_count - 1;
                    }
                    KeyCode::Left | KeyCode::Backspace => {
                        self.logs_focus = LogsFocus::Detail;
                    }
                    _ => {}
                }
            }
        }
    }

    fn handle_scroll_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.scroll.get().offset() == 0 {
                    self.focus_layer = DetailFocusLayer::MainTabs;
                } else {
                    self.with_scroll(|s| {
                        s.scroll_by(-1);
                    });
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.with_scroll(|s| {
                    s.scroll_by(1);
                });
            }
            KeyCode::PageUp => {
                self.with_scroll(|s| {
                    s.page_up();
                });
            }
            KeyCode::PageDown => {
                self.with_scroll(|s| {
                    s.page_down();
                });
            }
            KeyCode::Home => {
                self.with_scroll(|s| {
                    s.home();
                });
            }
            KeyCode::End => {
                self.with_scroll(|s| {
                    s.end();
                });
            }
            _ => {}
        }
    }

    fn handle_raw_key(&mut self, key: KeyEvent) {
        let Some(view) = self.current.as_ref() else {
            return;
        };
        let rows = raw_rows_for_view(view);
        if rows.is_empty() {
            return;
        }
        if self.raw_row >= rows.len() {
            self.raw_row = rows.len() - 1;
        }
        match key.code {
            KeyCode::Up => {
                if self.raw_row == 0 {
                    self.focus_layer = DetailFocusLayer::MainTabs;
                } else {
                    self.raw_row -= 1;
                }
            }
            KeyCode::Down if self.raw_row + 1 < rows.len() => {
                self.raw_row += 1;
            }
            KeyCode::Down => {}
            KeyCode::Char('k') => {
                self.raw_row = (self.raw_row + rows.len() - 1) % rows.len();
            }
            KeyCode::Char('j') => {
                self.raw_row = (self.raw_row + 1) % rows.len();
            }
            KeyCode::Home => self.raw_row = 0,
            KeyCode::End => self.raw_row = rows.len() - 1,
            _ => {}
        }
    }

    fn raw_rows(&self) -> Vec<OverviewRow> {
        self.current
            .as_ref()
            .map(raw_rows_for_view)
            .unwrap_or_default()
    }

    /// Re-send the current tx hash on the feed when it is pending.
    /// The background task re-runs the full pipeline, which refreshes
    /// the Asset Changes / State Changes tabs against the latest
    /// block. No-op for mined txs. See plan 12.6.3.
    fn resimulate(&mut self) {
        let is_pending = self
            .current
            .as_ref()
            .map(|v| v.tx.is_pending())
            .unwrap_or(false);
        if !is_pending {
            return;
        }
        if self.feed.input_tx.send(self.hash).is_ok() {
            self.resimulate_count = self.resimulate_count.saturating_add(1);
            // Reset enrichment status to `Pending` so the tabs show
            // the spinner text while the fresh simulation is on its
            // way. The next channel drain will overwrite both.
            if let Some(view) = self.current.as_mut() {
                view.asset_changes = LoadStatus::Pending;
                view.state_diff = LoadStatus::Pending;
            }
        }
    }

    fn copy_selected(&mut self) {
        let value = match self.active_tab {
            TxTab::Overview => self
                .overview_selected_copy_value()
                .or_else(|| self.current.as_ref().map(|v| v.tx.hash.to_hex())),
            TxTab::Logs => self.current.as_ref().and_then(|view| {
                let log = view.decoded_logs.get(self.logs_selected)?;
                let fields = self.log_fields(log);
                match self.logs_focus {
                    LogsFocus::Detail => fields.get(self.logs_field).map(|f| f.copy_value.clone()),
                    LogsFocus::List => fields.first().map(|f| f.copy_value.clone()),
                    LogsFocus::Raw => log_raw_dump(log)
                        .lines()
                        .nth(self.logs_raw_line)
                        .map(raw_log_line_copy_value),
                }
            }),
            TxTab::Raw => self
                .raw_rows()
                .get(self.raw_row)
                .map(|r| r.copy_value.clone()),
            TxTab::Internal | TxTab::AssetChanges | TxTab::StateChanges => {
                self.current.as_ref().map(|v| v.tx.hash.to_hex())
            }
        };
        if let Some(v) = value {
            if let Some(services) = self.cursor_services.as_ref() {
                // When services are wired, ship the same string the
                // legacy `last_copied_value` sink stores to the real
                // clipboard too. Failing copies are swallowed by the
                // adapter (headless fallback).
                services.copy(&NavigableValue::Plain(v.clone()));
            }
            self.last_copied_value = Some(v);
        }
    }
}

impl TxDetailScreen {
    fn body_block_border(&self) -> Style {
        let palette = PalettePreset::DarkDefault.palette();
        detail_body_border_style(self.focus_layer, &palette)
    }

    fn logs_pane_border(&self, pane_focused: bool) -> Style {
        let palette = PalettePreset::DarkDefault.palette();
        if self.focus_layer == DetailFocusLayer::Content && pane_focused {
            detail_body_border_style(DetailFocusLayer::Content, &palette)
        } else {
            detail_body_border_style(DetailFocusLayer::MainTabs, &palette)
        }
    }

    fn render_body(&self, frame: &mut Frame<'_>, area: Rect) {
        let Some(view) = self.current.as_ref() else {
            frame.render_widget(
                Paragraph::new("Loading...").block(
                    RatBlock::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_block_border())
                        .title("Overview"),
                ),
                area,
            );
            return;
        };
        match self.active_tab {
            TxTab::Overview => self.render_overview(frame, area, view),
            TxTab::Logs => self.render_logs(frame, area, view),
            TxTab::Internal => {
                self.render_scrollable(frame, area, "Internal", internal_body(&view.call_tree))
            }
            TxTab::AssetChanges => {
                self.render_scrollable(frame, area, "Asset Changes", asset_changes_tab_body(view))
            }
            TxTab::StateChanges => self.render_scrollable(
                frame,
                area,
                "State Changes",
                state_changes_body(&view.state_diff),
            ),
            TxTab::Raw => self.render_raw_tab(frame, area, view),
        }
    }

    fn render_overview(&self, frame: &mut Frame<'_>, area: Rect, _view: &TxView) {
        let rows = self.overview_rows();
        let lines: Vec<Line<'static>> = rows
            .iter()
            .enumerate()
            .map(|(idx, row)| overview_line(row, idx == self.overview_row))
            .collect();
        let total = lines.len() as u16;
        let viewport = area.height.saturating_sub(2);
        self.with_scroll(|s| s.set_dimensions(total, viewport));
        let offset = self.scroll.get().offset();
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((offset, 0))
                .block(
                    RatBlock::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_block_border())
                        .title("Overview"),
                ),
            area,
        );
    }

    fn render_logs(&self, frame: &mut Frame<'_>, area: Rect, view: &TxView) {
        let list_bb = self.logs_pane_border(matches!(self.logs_focus, LogsFocus::List));
        if view.decoded_logs.is_empty() {
            frame.render_widget(
                Paragraph::new("No logs emitted.").block(
                    RatBlock::default()
                        .borders(Borders::ALL)
                        .border_style(list_bb)
                        .title("Logs"),
                ),
                area,
            );
            return;
        }
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);

        let items: Vec<ListItem<'_>> = view
            .decoded_logs
            .iter()
            .enumerate()
            .map(|(idx, log)| ListItem::new(log_summary(idx, log)))
            .collect();
        let list = List::new(items)
            .block(
                RatBlock::default()
                    .borders(Borders::ALL)
                    .border_style(list_bb)
                    .title("Logs"),
            )
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::Indexed(238))
                    .fg(Color::White),
            )
            .highlight_symbol("> ");
        let mut list_state = ListState::default();
        list_state.select(Some(self.logs_selected));
        frame.render_stateful_widget(list, chunks[0], &mut list_state);

        let log = &view.decoded_logs[self.logs_selected];
        let fields = self.log_fields(log);
        let decoded_bb = self.logs_pane_border(matches!(self.logs_focus, LogsFocus::Detail));
        let raw_bb = self.logs_pane_border(matches!(self.logs_focus, LogsFocus::Raw));
        let detail_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(chunks[1]);

        let decoded_lines: Vec<Line<'static>> = fields
            .iter()
            .enumerate()
            .map(|(idx, field)| {
                log_field_line(field, {
                    matches!(self.logs_focus, LogsFocus::Detail) && idx == self.logs_field
                })
            })
            .collect();
        let n_decoded = decoded_lines.len() as u16;
        let decoded_viewport = detail_split[0].height.saturating_sub(2);
        self.with_logs_decoded_scroll(|s| {
            s.set_dimensions(n_decoded, decoded_viewport);
            s.scroll_row_into_view(self.logs_field as u16);
        });
        let decoded_offset = self.logs_decoded_scroll.get().offset();
        frame.render_widget(
            Paragraph::new(decoded_lines)
                .wrap(Wrap { trim: false })
                .scroll((decoded_offset, 0))
                .block(
                    RatBlock::default()
                        .borders(Borders::ALL)
                        .border_style(decoded_bb)
                        .title("Decoded"),
                ),
            detail_split[0],
        );

        let raw_text = log_raw_dump(log);
        let line_strings: Vec<&str> = raw_text.lines().collect();
        let n_lines = line_strings.len().max(1);
        let sel = self.logs_raw_line.min(n_lines.saturating_sub(1));
        let raw_viewport = detail_split[1].height.saturating_sub(2);
        self.with_logs_raw_scroll(|s| {
            s.set_dimensions(n_lines as u16, raw_viewport);
            s.scroll_row_into_view(sel as u16);
        });
        let raw_offset = self.logs_raw_scroll.get().offset();
        let highlight_raw = matches!(self.logs_focus, LogsFocus::Raw);
        let raw_styled: Vec<Line<'static>> = if line_strings.is_empty() {
            vec![Line::from("(empty)")]
        } else {
            line_strings
                .into_iter()
                .enumerate()
                .map(|(i, line)| {
                    let selected = highlight_raw && i == sel;
                    let style = if selected {
                        Style::default()
                            .bg(Color::Indexed(238))
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    Line::from(Span::styled(line.to_string(), style))
                })
                .collect()
        };
        frame.render_widget(
            Paragraph::new(raw_styled)
                .wrap(Wrap { trim: false })
                .scroll((raw_offset, 0))
                .block(
                    RatBlock::default()
                        .borders(Borders::ALL)
                        .border_style(raw_bb)
                        .title("Raw log"),
                ),
            detail_split[1],
        );
    }

    fn render_raw_tab(&self, frame: &mut Frame<'_>, area: Rect, view: &TxView) {
        let rows = raw_rows_for_view(view);
        if rows.is_empty() {
            frame.render_widget(
                Paragraph::new("(empty)").block(
                    RatBlock::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_block_border())
                        .title("Raw"),
                ),
                area,
            );
            return;
        }
        let lines: Vec<Line<'static>> = rows
            .iter()
            .enumerate()
            .map(|(idx, row)| overview_line(row, idx == self.raw_row))
            .collect();
        let total = lines.len() as u16;
        let viewport = area.height.saturating_sub(2);
        self.with_scroll(|s| s.set_dimensions(total, viewport));
        let offset = self.scroll.get().offset();
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((offset, 0))
                .block(
                    RatBlock::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_block_border())
                        .title("Raw JSON"),
                ),
            area,
        );
    }

    fn render_scrollable(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        title: &'static str,
        body: String,
    ) {
        let content_lines = body.lines().count() as u16;
        let viewport = area.height.saturating_sub(2);
        self.with_scroll(|s| s.set_dimensions(content_lines, viewport));
        let offset = self.scroll.get().offset();
        frame.render_widget(
            Paragraph::new(body)
                .wrap(Wrap { trim: false })
                .scroll((offset, 0))
                .block(
                    RatBlock::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_block_border())
                        .title(title),
                ),
            area,
        );
    }
}

fn overview_rows(view: &TxView) -> Vec<OverviewRow> {
    let tx = &view.tx;
    let mut rows = Vec::new();

    rows.push(OverviewRow {
        label: "Hash",
        display: tx.hash.to_hex(),
        copy_value: tx.hash.to_hex(),
        raw_hint: None,
    });

    let status_display = match &tx.status {
        TxStatus::Success => "success".to_string(),
        TxStatus::Failed { reason: Some(r) } => format!("failed - {r}"),
        TxStatus::Failed { reason: None } => "failed".to_string(),
        TxStatus::Pending => "pending".to_string(),
    };
    rows.push(OverviewRow {
        label: "Status",
        display: status_display.clone(),
        copy_value: status_display,
        raw_hint: None,
    });

    let block_display = tx
        .block_number
        .map(|b| format!("#{}", b.value()))
        .unwrap_or_else(|| "(pending)".to_string());
    let block_copy = tx
        .block_number
        .map(|b| b.value().to_string())
        .unwrap_or_else(|| "(pending)".to_string());
    rows.push(OverviewRow {
        label: "Block",
        display: block_display,
        copy_value: block_copy,
        raw_hint: None,
    });

    let idx_display = tx
        .tx_index
        .map(|i| i.to_string())
        .unwrap_or_else(|| "-".to_string());
    rows.push(OverviewRow {
        label: "Index",
        display: idx_display.clone(),
        copy_value: idx_display,
        raw_hint: None,
    });

    rows.push(OverviewRow {
        label: "From",
        display: tx.from.to_hex(),
        copy_value: tx.from.to_hex(),
        raw_hint: None,
    });

    let (to_display, to_copy) = match tx.to {
        Some(addr) => (addr.to_hex(), addr.to_hex()),
        None => ("(contract creation)".to_string(), String::new()),
    };
    rows.push(OverviewRow {
        label: "To",
        display: to_display,
        copy_value: to_copy,
        raw_hint: None,
    });

    rows.push(OverviewRow {
        label: "Value",
        display: humanize_eth(tx.value),
        copy_value: tx.value.value().to_string(),
        raw_hint: Some(format!("raw: {} wei", tx.value.value())),
    });

    let gas_used_display = match tx.gas_used {
        Some(g) => humanize_gas_units(g),
        None => "-".to_string(),
    };
    let gas_used_copy = tx
        .gas_used
        .map(|g| g.to_string())
        .unwrap_or_else(|| "-".to_string());
    rows.push(OverviewRow {
        label: "Gas used",
        display: format!(
            "{gas_used_display} / {gas_limit}",
            gas_limit = humanize_gas_units(tx.gas_limit)
        ),
        copy_value: gas_used_copy,
        raw_hint: tx.gas_used.map(|g| format!("raw: {g} gas")),
    });

    rows.push(OverviewRow {
        label: "Gas price",
        display: humanize_gwei(tx.gas_price),
        copy_value: tx.gas_price.value().to_string(),
        raw_hint: Some(format!("raw: {} wei", tx.gas_price.value())),
    });

    let (fee_display, fee_copy, fee_hint) = match tx.fee_paid() {
        Some(w) => (
            humanize_eth(w),
            w.value().to_string(),
            Some(format!("raw: {} wei", w.value())),
        ),
        None => ("(pending)".to_string(), "(pending)".to_string(), None),
    };
    rows.push(OverviewRow {
        label: "Fee paid",
        display: fee_display,
        copy_value: fee_copy,
        raw_hint: fee_hint,
    });

    rows.push(OverviewRow {
        label: "Nonce",
        display: tx.nonce.to_string(),
        copy_value: tx.nonce.to_string(),
        raw_hint: None,
    });

    rows.push(OverviewRow {
        label: "Type",
        display: tx.tx_type.label().to_string(),
        copy_value: tx.tx_type.label().to_string(),
        raw_hint: None,
    });

    let method = method_line(view);
    let method_copy = match view.decoded_method.as_ref() {
        Some(DecodedMethod { signature, .. }) => signature.clone(),
        None => match view.tx.selector() {
            Some(sel) => format!("0x{}", hex::encode(sel)),
            None if view.tx.input.is_empty() => String::new(),
            None => format!("0x{}", hex::encode(&view.tx.input)),
        },
    };
    rows.push(OverviewRow {
        label: "Method",
        display: method,
        copy_value: method_copy,
        raw_hint: None,
    });

    rows
}

fn overview_line(row: &OverviewRow, selected: bool) -> Line<'static> {
    let label = format!("{:<10} ", row.label);
    let primary = row.display.clone();
    let base_style = if selected {
        Style::default()
            .bg(Color::Indexed(238))
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let mut spans = vec![
        Span::styled(label, base_style),
        Span::styled(primary, base_style),
    ];
    if selected && let Some(hint) = &row.raw_hint {
        spans.push(Span::styled(
            format!("    {hint}"),
            Style::default().fg(Color::Gray),
        ));
    }
    Line::from(spans)
}

fn method_line(view: &TxView) -> String {
    match view.decoded_method.as_ref() {
        Some(DecodedMethod { signature, source }) => match source {
            SignatureSource::ProxyAbi { implementation, .. } => format!(
                "{signature} (decoded via implementation {})",
                short_address(implementation)
            ),
            other => format!("{signature} ({tag})", tag = other.tag()),
        },
        None => match view.tx.selector() {
            Some(sel) => format!("0x{} (unknown)", hex::encode(sel)),
            None if view.tx.input.is_empty() => "(empty)".to_string(),
            None => format!("0x{} (unknown)", hex::encode(&view.tx.input)),
        },
    }
}

/// Short-hand form used next to ProxyAbi decoding: keep the leading
/// `0x` + 4 bytes and the last 2 bytes so the user can correlate the
/// line with the real address without eating the whole row.
fn short_address(addr: &crate::domain::Address) -> String {
    let hex = addr.to_hex();
    if hex.len() <= 12 {
        return hex;
    }
    format!("{}…{}", &hex[..8], &hex[hex.len() - 4..])
}

/// `Transfer(address,address,uint256)` topic hash.
const ERC20_TRANSFER_TOPIC0: [u8; 32] = [
    0xdd, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d, 0xaa,
    0x95, 0x2b, 0xa7, 0xf1, 0x63, 0xc4, 0xa1, 0x16, 0x28, 0xf5, 0x5a, 0x4d, 0xf5, 0x23, 0xb3, 0xef,
];

fn log_raw_dump(log: &DecodedLog) -> String {
    let mut s = format!("contract: {}\n", log.raw.address.to_hex());
    for (i, topic) in log.raw.topics.iter().enumerate() {
        s.push_str(&format!("topic{i}: 0x{}\n", hex::encode(topic)));
    }
    s.push_str(&format!("data: 0x{}", hex::encode(&log.raw.data)));
    s
}

/// Clipboard payload for one line of [`log_raw_dump`]: the value after
/// `label: ` only (plan/18 Slice F).
fn raw_log_line_copy_value(line: &str) -> String {
    line.trim()
        .split_once(": ")
        .map(|(_, rest)| rest.trim().to_string())
        .unwrap_or_else(|| line.trim().to_string())
}

fn raw_rows_for_view(view: &TxView) -> Vec<OverviewRow> {
    let raw = view.tx.raw_json.as_str();
    if raw.trim().is_empty() {
        return vec![OverviewRow {
            label: "line",
            display: "(empty)".into(),
            copy_value: String::new(),
            raw_hint: None,
        }];
    }
    raw.lines()
        .enumerate()
        .map(|(i, line)| OverviewRow {
            label: "line",
            display: format!("{i}: {line}"),
            copy_value: line.to_string(),
            raw_hint: None,
        })
        .collect()
}

fn asset_changes_tab_body(view: &TxView) -> String {
    let mut out = derived_transfers_text(view);
    let addon = simulation_addon_text(&view.asset_changes);
    if !addon.is_empty() {
        out.push_str("\n\n");
        out.push_str(&addon);
    }
    out
}

fn derived_transfers_text(view: &TxView) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push("Receipt transfers (native ETH + ERC-20 Transfer logs)".to_string());
    let tx = &view.tx;
    if tx.value.value() > 0 {
        match tx.to {
            Some(to) => lines.push(format!(
                "{}  ──► {}  {}",
                short_address(&tx.from),
                short_address(&to),
                humanize_eth(tx.value),
            )),
            None => lines.push(format!(
                "{}  ──► (contract creation)  {}",
                short_address(&tx.from),
                humanize_eth(tx.value),
            )),
        }
    }
    for log in &view.decoded_logs {
        if let Some(line) = erc20_transfer_line(log) {
            lines.push(line);
        }
    }
    if lines.len() == 1 {
        lines.push(
            "(no native ETH in `value` and no ERC-20 Transfer events in this receipt)".to_string(),
        );
    }
    lines.join("\n")
}

fn simulation_addon_text(status: &LoadStatus<Vec<AssetChange>>) -> String {
    match status {
        LoadStatus::Pending => "Alchemy simulateAssetChanges: loading…".to_string(),
        LoadStatus::Unsupported => {
            "Alchemy simulateAssetChanges: unavailable on this chain / tier.".to_string()
        }
        LoadStatus::Failed(msg) => format!("Alchemy simulateAssetChanges failed: {msg}"),
        LoadStatus::Loaded(ch) if ch.is_empty() => String::new(),
        LoadStatus::Loaded(changes) => {
            let mut s = String::from("── Simulation extras (alchemy) ──\n");
            s.push_str(&format_simulated_changes(changes));
            s
        }
    }
}

fn erc20_transfer_line(log: &DecodedLog) -> Option<String> {
    let t0 = log.raw.topics.first()?;
    if *t0 != ERC20_TRANSFER_TOPIC0 || log.raw.topics.len() < 3 {
        return None;
    }
    let mut from_b = [0u8; 20];
    from_b.copy_from_slice(&log.raw.topics[1][12..]);
    let from = Address::from_bytes(from_b);
    let mut to_b = [0u8; 20];
    to_b.copy_from_slice(&log.raw.topics[2][12..]);
    let to = Address::from_bytes(to_b);
    let amount_str = erc20_transfer_amount_display(&log.raw.data);
    Some(format!(
        "{}  ──► {}  {}  (token {})",
        short_address(&from),
        short_address(&to),
        amount_str,
        short_address(&log.raw.address),
    ))
}

fn erc20_transfer_amount_display(data: &[u8]) -> String {
    match transfer_data_u128(data) {
        Some(v) => format!("{} (assumed 18 decimals)", humanize_token_units(v, 18)),
        None => format!("0x{}", hex::encode(data)),
    }
}

fn transfer_data_u128(data: &[u8]) -> Option<u128> {
    if data.is_empty() {
        return Some(0);
    }
    if data.len() < 32 {
        return None;
    }
    let w: &[u8; 32] = data[..32].try_into().ok()?;
    if w[..16].iter().any(|&b| b != 0) {
        return None;
    }
    let mut b = [0u8; 16];
    b.copy_from_slice(&w[16..]);
    Some(u128::from_be_bytes(b))
}

fn format_simulated_changes(changes: &[AssetChange]) -> String {
    let mut out = String::new();
    for (idx, change) in changes.iter().enumerate() {
        let kind = match change.kind {
            AssetChangeKind::Transfer => "TRANSFER",
            AssetChangeKind::Approve => "APPROVE",
            AssetChangeKind::Other => "OTHER",
        };
        let asset = match &change.asset {
            AssetKind::Native => "ETH (native)".to_string(),
            AssetKind::Erc20 {
                symbol, contract, ..
            } => {
                format!("{symbol} @ {}", contract.to_hex())
            }
            AssetKind::Erc721 {
                symbol,
                contract,
                token_id,
            } => {
                format!("{symbol} #{token_id} @ {}", contract.to_hex())
            }
            AssetKind::Erc1155 {
                symbol,
                contract,
                token_id,
            } => {
                format!("{symbol} id={token_id} @ {}", contract.to_hex())
            }
        };
        let from = change
            .from
            .map(|a| a.to_hex())
            .unwrap_or_else(|| "(mint)".to_string());
        let to = change
            .to
            .map(|a| a.to_hex())
            .unwrap_or_else(|| "(burn)".to_string());
        out.push_str(&format!(
            "#{idx}  {kind}  {asset}\n  from: {from}\n  to:   {to}\n  amount: {amount}\n\n",
            amount = humanize_wei_if_native(change.kind, &change.asset, change.amount),
        ));
    }
    out
}

fn log_summary(idx: usize, log: &DecodedLog) -> String {
    match log.signature.as_ref() {
        Some(sig) => format!("#{idx}  {}", sig.signature),
        None => match log.raw.topics.first() {
            Some(topic) => format!("#{idx}  0x{}...", hex::encode(&topic[..4])),
            None => format!("#{idx}  (anonymous)"),
        },
    }
}

/// Produce the field list for a log (plan 13.4, 12.6.4).
///
/// Three paths, in priority order:
///
/// 1. **ABI parsed** (`DecodedSignature::parsed = Some`): honour the
///    real `indexed` flag per parameter and align `topics[1..]`
///    onto indexed args and `data` words onto non-indexed args.
///    This matches the event exactly even when the indexed args
///    are not the leading positional ones (e.g. custom mixed
///    events).
/// 2. **Signature resolved via directory** (no `parsed`): best-
///    effort heuristic — assume the first N positional types (as
///    parsed from the textual signature) are the indexed ones.
///    This matches canonical ERC20/ERC721 events in practice.
/// 3. **No signature at all**: raw-hex per topic / data slot.
fn log_fields(log: &DecodedLog) -> Vec<LogField> {
    let mut fields = Vec::new();
    let topic0_hex = match log.raw.topics.first() {
        Some(t) => format!("0x{}", hex::encode(t)),
        None => "(anonymous)".to_string(),
    };
    match log.signature.as_ref() {
        Some(sig) => {
            fields.push(LogField {
                label: "event".to_string(),
                display: sig.signature.clone(),
                copy_value: sig.signature.clone(),
                raw_hint: Some(format!("topic0: {topic0_hex}")),
            });
            if let Some(parsed) = sig.parsed.as_ref() {
                push_abi_fields(&mut fields, parsed, &log.raw);
            } else {
                push_heuristic_fields(&mut fields, &sig.signature, &log.raw);
            }
        }
        None => {
            for (i, topic) in log.raw.topics.iter().enumerate() {
                let raw = format!("0x{}", hex::encode(topic));
                fields.push(LogField {
                    label: format!("topic{i}"),
                    display: raw.clone(),
                    copy_value: raw,
                    raw_hint: None,
                });
            }
            let data_hex = format!("0x{}", hex::encode(&log.raw.data));
            fields.push(LogField {
                label: "data".to_string(),
                display: data_hex.clone(),
                copy_value: data_hex,
                raw_hint: None,
            });
        }
    }
    fields
}

/// Align topics / data onto an ABI-parsed event: indexed params get
/// `topics[1..]`, non-indexed params get consecutive data words in
/// ABI order.
fn push_abi_fields(fields: &mut Vec<LogField>, parsed: &EventAbi, raw: &LogEntry) {
    let data_words = split_data_words(&raw.data);
    let mut indexed_cursor = 0usize;
    let mut data_cursor = 0usize;
    for (idx, param) in parsed.params.iter().enumerate() {
        let label_name = if param.name.is_empty() {
            format!("arg{idx}")
        } else {
            param.name.clone()
        };
        if param.indexed {
            // `topics[0]` is the event selector; indexed args live
            // in `topics[1..]`.
            let topic = raw.topics.get(1 + indexed_cursor).copied();
            indexed_cursor += 1;
            let (display, copy_value, raw_hex) = match topic {
                Some(t) => {
                    let (d, c) = decode_word(Some(param.type_.as_str()), &t);
                    (d, c, format!("0x{}", hex::encode(t)))
                }
                None => ("(missing topic)".to_string(), String::new(), String::new()),
            };
            fields.push(LogField {
                label: format!("{label_name} ({}, indexed)", param.type_),
                display,
                copy_value,
                raw_hint: if raw_hex.is_empty() {
                    None
                } else {
                    Some(format!("raw: {raw_hex}"))
                },
            });
        } else {
            let word = data_words.get(data_cursor).copied();
            data_cursor += 1;
            let (display, copy_value, raw_hex) = match word {
                Some(w) => {
                    let (d, c) = decode_word(Some(param.type_.as_str()), &w);
                    (d, c, format!("0x{}", hex::encode(w)))
                }
                None => (
                    "(missing data word)".to_string(),
                    String::new(),
                    String::new(),
                ),
            };
            fields.push(LogField {
                label: format!("{label_name} ({})", param.type_),
                display,
                copy_value,
                raw_hint: if raw_hex.is_empty() {
                    None
                } else {
                    Some(format!("raw: {raw_hex}"))
                },
            });
        }
    }
    // Leftover data words: surface them so the user can still copy
    // them even when the ABI shape disagrees with the payload.
    let aligned = data_words.len() * 32;
    if aligned < raw.data.len() {
        let tail = &raw.data[aligned..];
        fields.push(LogField {
            label: "data (tail)".to_string(),
            display: format!("0x{}", hex::encode(tail)),
            copy_value: format!("0x{}", hex::encode(tail)),
            raw_hint: None,
        });
    }
}

/// Fallback for signature-directory hits: parse positional types
/// out of the signature text and assume the first N are indexed.
/// Mirrors the previous behaviour of the Logs tab.
fn push_heuristic_fields(fields: &mut Vec<LogField>, signature: &str, raw: &LogEntry) {
    let arg_types = parse_signature_args(signature);
    let indexed_count = raw.topics.len().saturating_sub(1);
    for (i, topic) in raw.topics.iter().skip(1).enumerate() {
        let ty = arg_types.get(i).map(String::as_str);
        let (display, copy_value) = decode_word(ty, topic);
        let raw_hex = format!("0x{}", hex::encode(topic));
        let label = match ty {
            Some(t) => format!("arg{i} ({t}, indexed)"),
            None => format!("topic{}", i + 1),
        };
        fields.push(LogField {
            label,
            display,
            copy_value,
            raw_hint: Some(format!("raw: {raw_hex}")),
        });
    }
    let data_words = split_data_words(&raw.data);
    for (i, word) in data_words.iter().enumerate() {
        let ty_index = indexed_count + i;
        let ty = arg_types.get(ty_index).map(String::as_str);
        let (display, copy_value) = decode_word(ty, word);
        let raw_hex = format!("0x{}", hex::encode(word));
        let label = match ty {
            Some(t) => format!("arg{ty_index} ({t})"),
            None => format!("data[{i}]"),
        };
        fields.push(LogField {
            label,
            display,
            copy_value,
            raw_hint: Some(format!("raw: {raw_hex}")),
        });
    }
    let aligned = data_words.len() * 32;
    if aligned < raw.data.len() {
        let tail = &raw.data[aligned..];
        fields.push(LogField {
            label: "data (tail)".to_string(),
            display: format!("0x{}", hex::encode(tail)),
            copy_value: format!("0x{}", hex::encode(tail)),
            raw_hint: None,
        });
    }
}

fn log_field_line(field: &LogField, selected: bool) -> Line<'static> {
    let label = format!("{:<22} ", field.label);
    let base_style = if selected {
        Style::default()
            .bg(Color::Indexed(238))
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    Line::from(vec![
        Span::styled(label, base_style),
        Span::styled(field.display.clone(), base_style),
    ])
}

/// Parse the positional argument types out of a function / event
/// signature text like `Transfer(address,address,uint256)`.
/// Nested tuples and array suffixes are preserved verbatim; the
/// caller's decoder falls back to raw hex for anything it cannot
/// decode.
fn parse_signature_args(sig: &str) -> Vec<String> {
    let start = match sig.find('(') {
        Some(p) => p + 1,
        None => return Vec::new(),
    };
    let end = match sig.rfind(')') {
        Some(p) => p,
        None => return Vec::new(),
    };
    if end <= start {
        return Vec::new();
    }
    let body = &sig[start..end];
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for ch in body.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                let t = current.trim().to_string();
                if !t.is_empty() {
                    out.push(t);
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let last = current.trim().to_string();
    if !last.is_empty() {
        out.push(last);
    }
    out
}

/// Split a log's `data` into 32-byte chunks. Any trailing bytes
/// below 32 are exposed by the caller as a separate "tail" field.
fn split_data_words(data: &[u8]) -> Vec<[u8; 32]> {
    let mut out = Vec::with_capacity(data.len() / 32);
    let mut i = 0;
    while i + 32 <= data.len() {
        let mut buf = [0u8; 32];
        buf.copy_from_slice(&data[i..i + 32]);
        out.push(buf);
        i += 32;
    }
    out
}

/// Best-effort decoder for a single 32-byte word. Returns
/// `(display, copy_value)` where `display` is how we show the
/// value on the screen (human-oriented) and `copy_value` is the
/// canonical string the `y` key puts on the clipboard.
fn decode_word(ty: Option<&str>, word: &[u8; 32]) -> (String, String) {
    let raw_hex = format!("0x{}", hex::encode(word));
    let Some(ty) = ty else {
        return (raw_hex.clone(), raw_hex);
    };
    if ty == "address" {
        let addr = &word[12..];
        let hex_addr = format!("0x{}", hex::encode(addr));
        return (hex_addr.clone(), hex_addr);
    }
    if ty == "bool" {
        let v = word.iter().any(|b| *b != 0);
        let s = if v {
            "true".to_string()
        } else {
            "false".to_string()
        };
        return (s.clone(), s);
    }
    if (ty.starts_with("uint") || ty.starts_with("int")) && word[..16].iter().all(|b| *b == 0) {
        let mut u = [0u8; 16];
        u.copy_from_slice(&word[16..]);
        let v = u128::from_be_bytes(u);
        let s = v.to_string();
        return (s.clone(), s);
    }
    (raw_hex.clone(), raw_hex)
}

fn humanize_wei_if_native(_kind: AssetChangeKind, asset: &AssetKind, amount: Wei) -> String {
    match asset {
        AssetKind::Native => humanize_eth(amount),
        _ => amount.value().to_string(),
    }
}

fn state_changes_body(status: &LoadStatus<StateDiff>) -> String {
    match status {
        LoadStatus::Pending => "Replaying transaction for state diff...".to_string(),
        LoadStatus::Unsupported => {
            "State-diff trace is unavailable on this chain / tier.".to_string()
        }
        LoadStatus::Failed(msg) => format!("Trace failed: {msg}"),
        LoadStatus::Loaded(diff) if diff.is_empty() => "No state changes recorded.".to_string(),
        LoadStatus::Loaded(diff) => render_state_diff(diff),
    }
}

fn render_state_diff(diff: &StateDiff) -> String {
    let mut out = String::new();
    for entry in &diff.entries {
        out.push_str(&render_address_diff(entry));
        out.push('\n');
    }
    out
}

fn render_address_diff(entry: &AddressStateDiff) -> String {
    let mut out = format!("{}\n", entry.address.to_hex());
    if entry.balance.is_change() {
        out.push_str(&format!("  balance: {}\n", render_diff(&entry.balance)));
    }
    if entry.nonce.is_change() {
        out.push_str(&format!("  nonce:   {}\n", render_diff(&entry.nonce)));
    }
    if entry.code.is_change() {
        out.push_str(&format!("  code:    {}\n", render_diff(&entry.code)));
    }
    for slot in &entry.storage {
        out.push_str(&format!(
            "  storage[{slot}] = {change}\n",
            slot = slot.slot,
            change = render_diff(&slot.change),
        ));
    }
    out
}

fn render_diff(change: &DiffChange) -> String {
    match change {
        DiffChange::Unchanged => "(unchanged)".to_string(),
        DiffChange::Added(v) => format!("+{v}"),
        DiffChange::Removed(v) => format!("-{v}"),
        DiffChange::Changed { from, to } => format!("{from} -> {to}"),
    }
}

fn short_hex(s: &str) -> String {
    if s.len() <= 14 {
        return s.to_string();
    }
    format!("{}...{}", &s[..8], &s[s.len() - 4..])
}

/// Render the Internal tab body for the given call-tree status.
/// Mirrors the status-based fallback used by the other tabs.
/// See `plan/4-tx-detail.md` section 12.6.5.
fn internal_body(status: &LoadStatus<CallNode>) -> String {
    match status {
        LoadStatus::Pending => "Fetching call tree...".to_string(),
        LoadStatus::Unsupported => {
            "Call tree is unavailable on this chain / tier (trace + debug namespaces disabled)."
                .to_string()
        }
        LoadStatus::Failed(msg) => format!("Trace failed: {msg}"),
        LoadStatus::Loaded(root) => render_call_tree(root),
    }
}

fn render_call_tree(root: &CallNode) -> String {
    let mut out = String::new();
    render_call_node(root, "", true, &mut out);
    out
}

/// Emit one line per node using indent + elbow glyphs, then recurse.
/// `prefix` carries the accumulated prefix for deeper levels;
/// `is_last` controls which elbow character to use. Plan 12.6.5
/// deliberately keeps this dead-simple (no Unicode box drawing):
/// ASCII is enough and keeps copy/paste readable.
fn render_call_node(node: &CallNode, prefix: &str, is_last: bool, out: &mut String) {
    let marker = if prefix.is_empty() {
        ""
    } else if is_last {
        "`- "
    } else {
        "|- "
    };
    let to = match node.to {
        Some(addr) => short_address(&addr),
        None => "(create)".to_string(),
    };
    let error = node
        .error
        .as_ref()
        .map(|e| format!(" !! {e}"))
        .unwrap_or_default();
    out.push_str(&format!(
        "{prefix}{marker}{kind} -> {to}  gas={gas}{error}\n",
        kind = node.kind.label(),
        gas = node.gas_used,
    ));
    let child_prefix = if prefix.is_empty() {
        String::new()
    } else if is_last {
        format!("{prefix}   ")
    } else {
        format!("{prefix}|  ")
    };
    let next_prefix = if prefix.is_empty() {
        "   ".to_string()
    } else {
        child_prefix
    };
    for (i, child) in node.children.iter().enumerate() {
        let last = i + 1 == node.children.len();
        render_call_node(child, &next_prefix, last, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ratatui::{Terminal, backend::TestBackend};

    use crate::domain::{
        Address, BlockNumber, Chain, LogEntry, Transaction, TxHash, TxStatus, TxType, Wei,
    };

    #[test]
    fn raw_log_line_copy_value_strips_label_prefix() {
        assert_eq!(raw_log_line_copy_value("topic0: 0x010203"), "0x010203");
        assert_eq!(
            raw_log_line_copy_value("contract: a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            "a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
        );
        assert_eq!(raw_log_line_copy_value("data: 0xabcd"), "0xabcd");
    }

    #[test]
    fn logs_decoded_scroll_nonzero_when_field_cursor_past_viewport() {
        let addr = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
        let raw = LogEntry {
            address: addr,
            topics: (0..26).map(|_| [7u8; 32]).collect(),
            data: vec![0u8; 32],
        };
        let decoded = DecodedLog {
            raw,
            signature: None,
        };
        let tx = Transaction {
            chain: Chain::Ethereum,
            hash: TxHash::from_hex(
                "0xabcd016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a7139401",
            )
            .unwrap(),
            status: TxStatus::Success,
            block_number: Some(BlockNumber::new(21_345_678)),
            block_hash: None,
            tx_index: Some(0),
            from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
            to: Some(addr),
            value: Wei::new(0),
            gas_price: Wei::new(0),
            gas_used: Some(0),
            gas_limit: 0,
            nonce: 0,
            tx_type: TxType::DynamicFee,
            input: Vec::new(),
            logs: Vec::new(),
            raw_json: "{}".to_string(),
        };
        let view = TxView {
            tx,
            decoded_logs: vec![decoded],
            decoded_method: None,
            call_tree: LoadStatus::Unsupported,
            asset_changes: LoadStatus::Unsupported,
            state_diff: LoadStatus::Unsupported,
        };
        let (feed, sender) = tx_feed();
        let mut screen = TxDetailScreen::loading(Chain::Ethereum, view.tx.hash, feed);
        sender.updates_tx.send(view).unwrap();
        screen.tick();
        screen.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        screen.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        for _ in 0..20 {
            screen.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        let backend = TestBackend::new(120, 28);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| screen.render(frame, frame.area()))
            .expect("draw");
        assert!(
            screen.logs_decoded_scroll_offset_for_test() > 0,
            "decoded pane should scroll when the field cursor sits past the viewport"
        );
    }

    #[test]
    fn logs_raw_scroll_nonzero_when_line_cursor_past_viewport() {
        let addr = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
        let raw = LogEntry {
            address: addr,
            topics: (0..22).map(|_| [3u8; 32]).collect(),
            data: vec![5u8; 32],
        };
        let decoded = DecodedLog {
            raw,
            signature: None,
        };
        let tx = Transaction {
            chain: Chain::Ethereum,
            hash: TxHash::from_hex(
                "0xabcd016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a7139401",
            )
            .unwrap(),
            status: TxStatus::Success,
            block_number: Some(BlockNumber::new(21_345_678)),
            block_hash: None,
            tx_index: Some(0),
            from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
            to: Some(addr),
            value: Wei::new(0),
            gas_price: Wei::new(0),
            gas_used: Some(0),
            gas_limit: 0,
            nonce: 0,
            tx_type: TxType::DynamicFee,
            input: Vec::new(),
            logs: Vec::new(),
            raw_json: "{}".to_string(),
        };
        let view = TxView {
            tx,
            decoded_logs: vec![decoded],
            decoded_method: None,
            call_tree: LoadStatus::Unsupported,
            asset_changes: LoadStatus::Unsupported,
            state_diff: LoadStatus::Unsupported,
        };
        let (feed, sender) = tx_feed();
        let mut screen = TxDetailScreen::loading(Chain::Ethereum, view.tx.hash, feed);
        sender.updates_tx.send(view).unwrap();
        screen.tick();
        screen.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        screen.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        screen.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        for _ in 0..16 {
            screen.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        let backend = TestBackend::new(120, 28);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| screen.render(frame, frame.area()))
            .expect("draw");
        assert!(
            screen.logs_raw_scroll_offset_for_test() > 0,
            "raw log pane should scroll when the line cursor sits past the viewport"
        );
    }

    #[test]
    fn previous_wraps_from_first_tab_to_last() {
        assert_eq!(TxTab::Overview.previous(), TxTab::Raw);
        assert_eq!(TxTab::Raw.next(), TxTab::Overview);
    }

    #[test]
    fn parse_signature_args_handles_event_signature() {
        assert_eq!(
            parse_signature_args("Transfer(address,address,uint256)"),
            vec!["address", "address", "uint256"]
        );
    }

    #[test]
    fn parse_signature_args_handles_empty_args() {
        assert!(parse_signature_args("Paused()").is_empty());
    }

    #[test]
    fn parse_signature_args_preserves_nested_tuples() {
        assert_eq!(
            parse_signature_args("Complex((uint256,uint256),bool)"),
            vec!["(uint256,uint256)", "bool"]
        );
    }

    #[test]
    fn decode_word_decodes_address_from_padded_word() {
        let mut word = [0u8; 32];
        word[12..]
            .copy_from_slice(&hex::decode("d8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap());
        let (display, copy) = decode_word(Some("address"), &word);
        assert_eq!(display, "0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
        assert_eq!(copy, display);
    }

    #[test]
    fn decode_word_decodes_small_uint() {
        let mut word = [0u8; 32];
        word[31] = 42;
        let (display, copy) = decode_word(Some("uint256"), &word);
        assert_eq!(display, "42");
        assert_eq!(copy, "42");
    }

    #[test]
    fn decode_word_falls_back_to_raw_hex_for_large_uint() {
        let mut word = [0u8; 32];
        // Set a bit above 128, forcing raw-hex fallback.
        word[0] = 1;
        let (display, _) = decode_word(Some("uint256"), &word);
        assert!(display.starts_with("0x"));
    }

    #[test]
    fn internal_body_renders_tree_with_ascii_indent() {
        use crate::domain::{Address, CallKind, CallNode, Wei};
        let root = CallNode {
            kind: CallKind::Call,
            from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
            to: Some(Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()),
            value: Wei::new(0),
            input: Vec::new(),
            output: Vec::new(),
            gas_used: 100,
            error: None,
            children: vec![CallNode {
                kind: CallKind::Staticcall,
                from: Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
                to: Some(Address::from_hex("0x1111111111111111111111111111111111111111").unwrap()),
                value: Wei::new(0),
                input: Vec::new(),
                output: Vec::new(),
                gas_used: 10,
                error: None,
                children: Vec::new(),
            }],
        };
        let body = internal_body(&LoadStatus::Loaded(root));
        assert!(body.contains("CALL ->"));
        assert!(body.contains("`- STATICCALL ->"));
    }

    #[test]
    fn internal_body_pending_message() {
        assert!(internal_body(&LoadStatus::Pending).starts_with("Fetching call tree"));
    }

    #[test]
    fn internal_body_unsupported_message() {
        assert!(internal_body(&LoadStatus::Unsupported).contains("unavailable"));
    }
}
