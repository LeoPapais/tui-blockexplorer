//! Transaction Detail screen.
//!
//! Surfaces Overview / Logs / Asset Changes / State Changes / Raw
//! tabs. See `plan/4-tx-detail.md` sections 12.3 (MVP), 12.4
//! (expansion) and 13 (follow-up fixes).
//!
//! Key layout:
//! - `Tab` / `Right` advance tabs; `Shift+Tab` / `Left` go back.
//!   Arrow keys switch tabs only when the focused widget did not
//!   claim them (see `Logs` detail pane below).
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
        format::{humanize_eth, humanize_gas_units, humanize_gwei},
        screen::{Command, Screen},
        scroll::ScrollState,
    },
    application::{DecodedLog, DecodedMethod, LoadStatus, TxView},
    domain::{
        AddressStateDiff, AssetChange, AssetChangeKind, AssetKind, Chain, DiffChange, StateDiff,
        TxHash, TxStatus, Wei,
    },
};

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
    AssetChanges,
    StateChanges,
    Raw,
}

impl TxTab {
    const ALL: [TxTab; 5] = [
        TxTab::Overview,
        TxTab::Logs,
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
            TxTab::AssetChanges => 2,
            TxTab::StateChanges => 3,
            TxTab::Raw => 4,
        }
    }

    fn label(self) -> &'static str {
        match self {
            TxTab::Overview => "Overview",
            TxTab::Logs => "Logs",
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
    current: Option<TxView>,
    feed: TxFeed,
    active_tab: TxTab,
    /// Row-level cursor for the Overview tab (plan 13.1).
    overview_row: usize,
    /// Left-pane log cursor for the Logs tab (plan 13.4).
    logs_selected: usize,
    /// Right-pane field cursor for the Logs tab (plan 13.4).
    logs_field: usize,
    /// Pane focus inside the Logs tab (plan 13.4).
    logs_focus: LogsFocus,
    /// Bounded scroll used by tabs whose content is rendered as a
    /// single `Paragraph` (plan 13.3). Wrapped in a `Cell` so the
    /// render path can refresh `content_height` / `viewport_height`
    /// through `&self` and `handle_key` can then clamp the offset
    /// against up-to-date dimensions.
    scroll: Cell<ScrollState>,
    /// Last value produced by the `y` "copy" binding. The real
    /// runtime wires a clipboard adapter on top; tests inspect the
    /// field directly.
    last_copied_value: Option<String>,
}

impl TxDetailScreen {
    #[must_use]
    pub fn loading(chain: Chain, hash: TxHash, feed: TxFeed) -> Self {
        let _ = feed.input_tx.send(hash);
        Self {
            chain,
            current: None,
            feed,
            active_tab: TxTab::Overview,
            overview_row: 0,
            logs_selected: 0,
            logs_field: 0,
            logs_focus: LogsFocus::List,
            scroll: Cell::new(ScrollState::new()),
            last_copied_value: None,
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

    fn drain_feed(&mut self) {
        while let Ok(view) = self.feed.updates_rx.try_recv() {
            self.current = Some(view);
            self.overview_row = 0;
            self.logs_selected = 0;
            self.logs_field = 0;
            self.logs_focus = LogsFocus::List;
            self.with_scroll(|s| s.reset());
        }
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

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(3),
            ])
            .split(area);

        let header = match self.current.as_ref() {
            Some(view) => format!("Tx {}", short_hex(&view.tx.hash.to_hex())),
            None => "Tx (loading...)".to_string(),
        };
        frame.render_widget(
            Paragraph::new(header).block(
                RatBlock::default()
                    .borders(Borders::ALL)
                    .title("Transaction"),
            ),
            chunks[0],
        );

        let titles: Vec<Line<'static>> = TxTab::ALL
            .iter()
            .map(|t| Line::from(format!(" {} ", t.label())))
            .collect();
        frame.render_widget(
            Tabs::new(titles)
                .select(self.active_tab.index())
                .block(RatBlock::default().borders(Borders::ALL).title("Tabs"))
                .divider(" ")
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::Indexed(238))
                        .fg(Color::White),
                ),
            chunks[1],
        );

        self.render_body(frame, chunks[2]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        // Global.
        match key.code {
            KeyCode::Char('q') => return Command::Quit,
            KeyCode::Esc => {
                if self.active_tab == TxTab::Logs && self.logs_focus == LogsFocus::Detail {
                    self.logs_focus = LogsFocus::List;
                    return Command::None;
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
        if is_back_tab {
            self.switch_tab(self.active_tab.previous());
            return Command::None;
        }
        if key.code == KeyCode::Tab {
            self.switch_tab(self.active_tab.next());
            return Command::None;
        }

        // `y` copies the selected row / field, regardless of tab.
        if key.code == KeyCode::Char('y') {
            self.copy_selected();
            return Command::None;
        }

        match self.active_tab {
            TxTab::Overview => self.handle_overview_key(key),
            TxTab::Logs => self.handle_logs_key(key),
            TxTab::AssetChanges | TxTab::StateChanges | TxTab::Raw => {
                self.handle_scroll_key(key);
            }
        }
        Command::None
    }

    fn tick(&mut self) -> Command {
        self.drain_feed();
        Command::None
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
        }
    }

    fn handle_overview_key(&mut self, key: KeyEvent) {
        let rows = self.overview_rows();
        if rows.is_empty() {
            return;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.overview_row = (self.overview_row + rows.len() - 1) % rows.len();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.overview_row = (self.overview_row + 1) % rows.len();
            }
            KeyCode::Left => self.switch_tab(self.active_tab.previous()),
            KeyCode::Right => self.switch_tab(self.active_tab.next()),
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
        match self.logs_focus {
            LogsFocus::List => match key.code {
                KeyCode::Up | KeyCode::Char('k') if log_count > 0 => {
                    self.logs_selected = (self.logs_selected + log_count - 1) % log_count;
                    self.logs_field = 0;
                }
                KeyCode::Down | KeyCode::Char('j') if log_count > 0 => {
                    self.logs_selected = (self.logs_selected + 1) % log_count;
                    self.logs_field = 0;
                }
                KeyCode::Left => self.switch_tab(self.active_tab.previous()),
                KeyCode::Right | KeyCode::Enter if log_count > 0 => {
                    self.logs_focus = LogsFocus::Detail;
                    self.logs_field = 0;
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
                        self.logs_field = (self.logs_field + 1) % fields.len();
                    }
                    KeyCode::Left | KeyCode::Backspace => {
                        self.logs_focus = LogsFocus::List;
                    }
                    _ => {}
                }
            }
        }
    }

    fn handle_scroll_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.with_scroll(|s| {
                    s.scroll_by(-1);
                });
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.with_scroll(|s| {
                    s.scroll_by(1);
                });
            }
            KeyCode::Left => self.switch_tab(self.active_tab.previous()),
            KeyCode::Right => self.switch_tab(self.active_tab.next()),
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
                }
            }),
            TxTab::AssetChanges | TxTab::StateChanges | TxTab::Raw => {
                self.current.as_ref().map(|v| v.tx.hash.to_hex())
            }
        };
        if let Some(v) = value {
            self.last_copied_value = Some(v);
        }
    }
}

impl TxDetailScreen {
    fn render_body(&self, frame: &mut Frame<'_>, area: Rect) {
        let Some(view) = self.current.as_ref() else {
            frame.render_widget(
                Paragraph::new("Loading...")
                    .block(RatBlock::default().borders(Borders::ALL).title("Overview")),
                area,
            );
            return;
        };
        match self.active_tab {
            TxTab::Overview => self.render_overview(frame, area, view),
            TxTab::Logs => self.render_logs(frame, area, view),
            TxTab::AssetChanges => self.render_scrollable(
                frame,
                area,
                "Asset Changes",
                asset_changes_body(&view.asset_changes),
            ),
            TxTab::StateChanges => self.render_scrollable(
                frame,
                area,
                "State Changes",
                state_changes_body(&view.state_diff),
            ),
            TxTab::Raw => self.render_scrollable(frame, area, "Raw", view.tx.raw_json.clone()),
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
                .block(RatBlock::default().borders(Borders::ALL).title("Overview")),
            area,
        );
    }

    fn render_logs(&self, frame: &mut Frame<'_>, area: Rect, view: &TxView) {
        if view.decoded_logs.is_empty() {
            frame.render_widget(
                Paragraph::new("No logs emitted.")
                    .block(RatBlock::default().borders(Borders::ALL).title("Logs")),
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
        let list_focused = matches!(self.logs_focus, LogsFocus::List);
        let list_title = if list_focused { "Logs *" } else { "Logs" };
        let list = List::new(items)
            .block(RatBlock::default().borders(Borders::ALL).title(list_title))
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
        let focused = matches!(self.logs_focus, LogsFocus::Detail);
        let lines: Vec<Line<'static>> = std::iter::once(Line::from(Span::styled(
            format!("addr: {}", log.raw.address.to_hex()),
            Style::default().fg(Color::Gray),
        )))
        .chain(
            fields
                .iter()
                .enumerate()
                .map(|(idx, field)| log_field_line(field, focused && idx == self.logs_field)),
        )
        .collect();
        let title = if focused {
            "Log detail *"
        } else {
            "Log detail"
        };
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .block(RatBlock::default().borders(Borders::ALL).title(title)),
            chunks[1],
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
                .block(RatBlock::default().borders(Borders::ALL).title(title)),
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
        Some(DecodedMethod { signature, source }) => {
            format!("{signature} ({tag})", tag = source.tag())
        }
        None => match view.tx.selector() {
            Some(sel) => format!("0x{} (unknown)", hex::encode(sel)),
            None if view.tx.input.is_empty() => "(empty)".to_string(),
            None => format!("0x{} (unknown)", hex::encode(&view.tx.input)),
        },
    }
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

/// Produce the field list for a log (plan 13.4).
///
/// - When a signature is available (ABI or sigdb), parse the
///   positional argument types out of the textual signature and
///   attempt to decode topics and data words into those types
///   (best effort: `address`, `uint*` up to `uint128`, `int*` up
///   to `int128`, and `bool`). Anything else renders as raw hex.
/// - Without a signature, each topic and data word shows up as
///   its own raw-hex field so the user can still copy it.
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
            let arg_types = parse_signature_args(&sig.signature);
            // topics[1..] map onto the first types (indexed args).
            // data words map onto the remaining types.
            let indexed_count = log.raw.topics.len().saturating_sub(1);
            for (i, topic) in log.raw.topics.iter().skip(1).enumerate() {
                let ty = arg_types.get(i).map(String::as_str);
                let (display, copy_value) = decode_word(ty, topic);
                let raw = format!("0x{}", hex::encode(topic));
                let label = match ty {
                    Some(t) => format!("arg{i} ({t}, indexed)"),
                    None => format!("topic{}", i + 1),
                };
                fields.push(LogField {
                    label,
                    display,
                    copy_value,
                    raw_hint: Some(format!("raw: {raw}")),
                });
            }
            // Data words for non-indexed args.
            let data_words = split_data_words(&log.raw.data);
            for (i, word) in data_words.iter().enumerate() {
                let ty_index = indexed_count + i;
                let ty = arg_types.get(ty_index).map(String::as_str);
                let (display, copy_value) = decode_word(ty, word);
                let raw = format!("0x{}", hex::encode(word));
                let label = match ty {
                    Some(t) => format!("arg{ty_index} ({t})"),
                    None => format!("data[{i}]"),
                };
                fields.push(LogField {
                    label,
                    display,
                    copy_value,
                    raw_hint: Some(format!("raw: {raw}")),
                });
            }
            // If data is not aligned to 32 bytes, show the tail.
            let aligned = data_words.len() * 32;
            if aligned < log.raw.data.len() {
                let tail = &log.raw.data[aligned..];
                fields.push(LogField {
                    label: "data (tail)".to_string(),
                    display: format!("0x{}", hex::encode(tail)),
                    copy_value: format!("0x{}", hex::encode(tail)),
                    raw_hint: None,
                });
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
    let mut spans = vec![
        Span::styled(label, base_style),
        Span::styled(field.display.clone(), base_style),
    ];
    if selected && let Some(hint) = &field.raw_hint {
        spans.push(Span::styled(
            format!("    {hint}"),
            Style::default().fg(Color::Gray),
        ));
    }
    Line::from(spans)
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

fn asset_changes_body(status: &LoadStatus<Vec<AssetChange>>) -> String {
    match status {
        LoadStatus::Pending => "Simulating asset changes...".to_string(),
        LoadStatus::Unsupported => {
            "Asset-change simulation is unavailable on this chain / tier.".to_string()
        }
        LoadStatus::Failed(msg) => format!("Simulation failed: {msg}"),
        LoadStatus::Loaded(changes) if changes.is_empty() => {
            "No asset changes detected.".to_string()
        }
        LoadStatus::Loaded(changes) => {
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
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
