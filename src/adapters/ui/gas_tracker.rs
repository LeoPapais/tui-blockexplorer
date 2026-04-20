//! Gas Tracker screen.
//!
//! The base layout was scaffolded under `plan/9-gas-tracker.md` §11.1;
//! the pause (`p`) + manual refresh (`Ctrl+R`), percentile histogram
//! (`p25 / p50 / p75`) and unit converter modal (`u`) follow-ups live
//! under §11.2, §11.3 and §11.4.

use std::collections::VecDeque;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::{
        field_cursor::{CursorDir, CursorServices, FieldCursor, FieldEntry},
        screen::{Command, Screen},
    },
    domain::{
        Chain, GasSnapshot, Gwei, NavigableValue,
        gas::{self, Percentiles, Unit},
    },
};

/// Rolling buffer size used for the percentile histogram. Matches the
/// "last 60 snapshots" figure in `plan/9-gas-tracker.md` §11.3.
pub const HISTOGRAM_WINDOW: usize = 60;

// ---------------------------------------------------------------------------
// Feed channel (MVP)
// ---------------------------------------------------------------------------

pub struct GasFeed {
    pub updates_rx: UnboundedReceiver<GasSnapshot>,
}

pub struct GasFeedSender {
    pub updates_tx: UnboundedSender<GasSnapshot>,
}

#[must_use]
pub fn gas_feed() -> (GasFeed, GasFeedSender) {
    let (tx, rx) = unbounded_channel();
    (GasFeed { updates_rx: rx }, GasFeedSender { updates_tx: tx })
}

// ---------------------------------------------------------------------------
// Refresh channel (§11.2)
// ---------------------------------------------------------------------------

/// Sender half of the Gas Tracker's refresh-kick channel. The screen
/// owns it and pushes a `()` every time the user presses `Ctrl+R`.
pub struct GasRefreshHandle {
    tx: UnboundedSender<()>,
}

impl GasRefreshHandle {
    /// Request a refresh. Dropped kicks (when the feed task is gone)
    /// are silently ignored — the command still bubbles up through
    /// `Command::Refresh` for the dispatcher to consume.
    pub fn kick(&self) {
        let _ = self.tx.send(());
    }
}

/// Receiver half of the Gas Tracker's refresh-kick channel. The feed
/// task selects over it alongside the polling timer.
pub struct GasRefreshListener {
    rx: UnboundedReceiver<()>,
}

impl GasRefreshListener {
    /// Non-blocking poll. Returns `Some(())` when a kick was queued,
    /// `None` otherwise.
    pub fn try_recv(&mut self) -> Option<()> {
        self.rx.try_recv().ok()
    }

    /// Await the next kick. Returns `None` when every sender has been
    /// dropped.
    pub async fn recv(&mut self) -> Option<()> {
        self.rx.recv().await
    }
}

/// Build a refresh-kick channel. The handle lives with the screen and
/// the listener with the feed task — see
/// [`crate::infra::gas_feed::spawn_with_refresh`].
#[must_use]
pub fn gas_refresh_channel() -> (GasRefreshHandle, GasRefreshListener) {
    let (tx, rx) = unbounded_channel();
    (GasRefreshHandle { tx }, GasRefreshListener { rx })
}

// ---------------------------------------------------------------------------
// Unit converter modal (§11.4)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ConverterState {
    input: String,
    from: Unit,
    to: Unit,
    result: Option<String>,
    error: Option<String>,
}

impl Default for ConverterState {
    fn default() -> Self {
        Self {
            input: String::new(),
            from: Unit::Ether,
            to: Unit::Gwei,
            result: None,
            error: None,
        }
    }
}

impl ConverterState {
    fn submit(&mut self) {
        match gas::convert_unit(&self.input, self.from, self.to) {
            Ok(value) => {
                self.result = Some(value);
                self.error = None;
            }
            Err(err) => {
                self.result = None;
                self.error = Some(err.to_string());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Screen
// ---------------------------------------------------------------------------

pub struct GasTrackerScreen {
    chain: Chain,
    current: Option<GasSnapshot>,
    feed: GasFeed,
    refresh_handle: Option<GasRefreshHandle>,
    paused: bool,
    /// Rolling `base_fee` buffer used for the histogram. Newest
    /// element last. Bounded to [`HISTOGRAM_WINDOW`].
    history: VecDeque<Gwei>,
    converter: Option<ConverterState>,
    cursor: FieldCursor,
    cursor_services: Option<CursorServices>,
}

impl GasTrackerScreen {
    /// Build a Gas Tracker screen subscribed to `feed`. If `initial`
    /// is set, the screen renders it straight away and replaces it
    /// with feed updates.
    #[must_use]
    pub fn new(chain: Chain, initial: Option<GasSnapshot>, feed: GasFeed) -> Self {
        // The rolling histogram buffer only tracks samples that flow
        // through the live feed. The primed `initial` snapshot is a
        // static display value (often the last `home_feed` pulse) and
        // is deliberately kept out of the buffer so the percentile
        // widget reads zero until the feed delivers its first live
        // sample.
        Self {
            chain,
            current: initial,
            feed,
            refresh_handle: None,
            paused: false,
            history: VecDeque::with_capacity(HISTOGRAM_WINDOW),
            converter: None,
            cursor: FieldCursor::new(),
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

    /// Reading-order list of navigable fields. Every gas-tier value
    /// is `Plain` — this screen does not contain any navigable
    /// addresses, hashes or block numbers.
    #[must_use]
    pub fn navigable_fields(&self) -> Vec<FieldEntry> {
        let mut out = Vec::new();
        if let Some(g) = self.current.as_ref() {
            out.push(FieldEntry::new(
                "gas_slow",
                NavigableValue::Plain(format!("{} gwei", g.slow.value())),
            ));
            out.push(FieldEntry::new(
                "gas_average",
                NavigableValue::Plain(format!("{} gwei", g.average.value())),
            ));
            out.push(FieldEntry::new(
                "gas_fast",
                NavigableValue::Plain(format!("{} gwei", g.fast.value())),
            ));
            out.push(FieldEntry::new(
                "gas_base_fee",
                NavigableValue::Plain(format!("{} gwei", g.base_fee.value())),
            ));
        }
        out
    }

    /// Current cursor state. Exposed for tests.
    #[must_use]
    pub const fn cursor(&self) -> &FieldCursor {
        &self.cursor
    }

    /// Attach a refresh handle. When set, `Ctrl+R` kicks the handle
    /// (so the feed task can refetch immediately) in addition to
    /// returning `Command::Refresh`.
    #[must_use]
    pub fn with_refresh_handle(mut self, handle: GasRefreshHandle) -> Self {
        self.refresh_handle = Some(handle);
        self
    }

    /// Chain this screen tracks.
    #[must_use]
    pub fn chain(&self) -> Chain {
        self.chain
    }

    /// Latest snapshot on the screen (updated by the feed task or
    /// primed at construction time).
    #[must_use]
    pub fn current(&self) -> Option<&GasSnapshot> {
        self.current.as_ref()
    }

    /// `true` when the user toggled pause. While paused the screen
    /// keeps draining the feed channel so the feed task never
    /// back-pressures, but incoming snapshots are dropped.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Percentiles of the `base_fee` rolling buffer. See
    /// `plan/9-gas-tracker.md` §11.3.
    #[must_use]
    pub fn percentiles(&self) -> Percentiles {
        let samples: Vec<Gwei> = self.history.iter().copied().collect();
        gas::percentiles(&samples)
    }

    /// `true` when the unit converter modal is open. See
    /// `plan/9-gas-tracker.md` §11.4.
    #[must_use]
    pub fn converter_open(&self) -> bool {
        self.converter.is_some()
    }

    /// Current text in the modal's input field. Empty string when
    /// the modal is closed.
    #[must_use]
    pub fn converter_input(&self) -> &str {
        self.converter.as_ref().map_or("", |c| c.input.as_str())
    }

    /// `(from, to)` unit pair currently selected in the modal.
    ///
    /// Returns `(Ether, Gwei)` — the default — when the modal is
    /// closed, so call sites do not need to branch on `Option`.
    #[must_use]
    pub fn converter_units(&self) -> (Unit, Unit) {
        self.converter
            .as_ref()
            .map_or((Unit::Ether, Unit::Gwei), |c| (c.from, c.to))
    }

    /// Latest successful conversion output.
    #[must_use]
    pub fn converter_result(&self) -> Option<&str> {
        self.converter.as_ref().and_then(|c| c.result.as_deref())
    }

    /// Latest validation error. Cleared on the next successful submit.
    #[must_use]
    pub fn converter_error(&self) -> Option<&str> {
        self.converter.as_ref().and_then(|c| c.error.as_deref())
    }

    fn drain_feed(&mut self) {
        while let Ok(update) = self.feed.updates_rx.try_recv() {
            if self.paused {
                // Drop: the screen froze on the last snapshot.
                continue;
            }
            self.record_snapshot(update);
        }
    }

    fn record_snapshot(&mut self, update: GasSnapshot) {
        if self.history.len() == HISTOGRAM_WINDOW {
            self.history.pop_front();
        }
        self.history.push_back(update.base_fee);
        self.current = Some(update);
    }

    fn dispatch_key_in_modal(&mut self, key: KeyEvent) -> Command {
        // These branches only run when `self.converter.is_some()`;
        // take a `&mut ConverterState` once to keep the borrow
        // checker happy.
        let Some(state) = self.converter.as_mut() else {
            // unreachable: dispatch_key_in_modal is gated on open().
            return Command::None;
        };
        match key.code {
            KeyCode::Esc => {
                self.converter = None;
                Command::None
            }
            KeyCode::Enter => {
                state.submit();
                Command::None
            }
            KeyCode::Backspace => {
                state.input.pop();
                Command::None
            }
            KeyCode::Left => {
                state.from = state.from.prev();
                Command::None
            }
            KeyCode::Right => {
                state.from = state.from.next();
                Command::None
            }
            KeyCode::Up => {
                state.to = state.to.prev();
                Command::None
            }
            KeyCode::Down => {
                state.to = state.to.next();
                Command::None
            }
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' || c == '-' => {
                state.input.push(c);
                Command::None
            }
            // Swallow anything else so the outer screen shortcuts
            // ('p' in particular) do not fire while the modal is
            // open.
            _ => Command::None,
        }
    }
}

impl Screen for GasTrackerScreen {
    fn title(&self) -> &str {
        "Gas Tracker"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(5),
                Constraint::Length(1),
            ])
            .split(area);

        let badge = if self.paused { "paused" } else { "live" };
        let header = format!("Gas Tracker — {} [{}]", self.chain.slug(), badge);
        let mut header_para =
            Paragraph::new(header).block(Block::default().borders(Borders::ALL).title("Header"));
        if self.paused {
            header_para = header_para.style(Style::default().add_modifier(Modifier::BOLD));
        }
        frame.render_widget(header_para, chunks[0]);

        let body = match self.current.as_ref() {
            Some(g) => format!(
                "Slow     {slow} gwei\n\
                 Average  {avg} gwei\n\
                 Fast     {fast} gwei\n\
                 \n\
                 Base fee {base} gwei\n\
                 Trend    {trend}",
                slow = g.slow.value(),
                avg = g.average.value(),
                fast = g.fast.value(),
                base = g.base_fee.value(),
                trend = format_trend(g),
            ),
            None => "Loading gas oracle...".to_string(),
        };
        frame.render_widget(
            Paragraph::new(body)
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title("Speeds")),
            chunks[1],
        );

        let p = self.percentiles();
        let histogram = format!(
            "p25 {p25} gwei  {bar_25}\n\
             p50 {p50} gwei  {bar_50}\n\
             p75 {p75} gwei  {bar_75}",
            p25 = p.p25.value(),
            p50 = p.p50.value(),
            p75 = p.p75.value(),
            bar_25 = histogram_bar(p.p25.value(), p.p75.value()),
            bar_50 = histogram_bar(p.p50.value(), p.p75.value()),
            bar_75 = histogram_bar(p.p75.value(), p.p75.value()),
        );
        frame.render_widget(
            Paragraph::new(histogram).wrap(Wrap { trim: false }).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Priority fee percentiles"),
            ),
            chunks[2],
        );

        let hint = " u unit converter   p pause   Ctrl+R refresh   Esc back   q quit ".to_string();
        frame.render_widget(
            Paragraph::new(hint).style(Style::default().add_modifier(Modifier::DIM)),
            chunks[3],
        );

        if self.converter.is_some() {
            self.render_converter(frame, area);
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        if self.converter.is_some() {
            return self.dispatch_key_in_modal(key);
        }

        // Ctrl+R takes precedence over plain 'r' so terminals that
        // send the key as a lowercase `r` with modifiers match too.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && let KeyCode::Char(c) = key.code
            && (c == 'r' || c == 'R')
        {
            self.paused = false;
            if let Some(handle) = self.refresh_handle.as_ref() {
                handle.kick();
            }
            return Command::Refresh;
        }

        match key.code {
            KeyCode::Char('q') => return Command::Quit,
            KeyCode::Esc => return Command::Pop,
            // Backspace deactivates the field cursor without popping
            // the screen. Moved off Esc so Esc always pops (see
            // plan/15-backlog.md §8.16).
            KeyCode::Backspace if self.cursor.is_active() => {
                self.cursor.deactivate();
                return Command::None;
            }
            _ => {}
        }

        let fields = self.navigable_fields();
        match key.code {
            KeyCode::Left => {
                self.cursor.move_in(fields.len(), CursorDir::Left);
                return Command::None;
            }
            KeyCode::Right => {
                self.cursor.move_in(fields.len(), CursorDir::Right);
                return Command::None;
            }
            KeyCode::Up => {
                self.cursor.move_in(fields.len(), CursorDir::Up);
                return Command::None;
            }
            KeyCode::Down => {
                self.cursor.move_in(fields.len(), CursorDir::Down);
                return Command::None;
            }
            KeyCode::Char('y') if self.cursor.is_active() => {
                if let (Some(entry), Some(services)) =
                    (self.cursor.current(&fields), self.cursor_services.as_ref())
                {
                    services.copy(&entry.value);
                }
                return Command::None;
            }
            // Default `y` (no active cursor) copies the first
            // navigable field so the shortcut always does something
            // useful. Mirrors the legacy copy behaviour on the other
            // detail screens.
            KeyCode::Char('y') => {
                if let (Some(entry), Some(services)) =
                    (fields.first(), self.cursor_services.as_ref())
                {
                    services.copy(&entry.value);
                }
                return Command::None;
            }
            KeyCode::Enter if self.cursor.is_active() => {
                if let (Some(entry), Some(services)) =
                    (self.cursor.current(&fields), self.cursor_services.as_ref())
                    && let Some(screen) = services.open(&entry.value)
                {
                    return Command::Push(screen);
                }
                return Command::None;
            }
            _ => {}
        }

        match key.code {
            KeyCode::Char('p') => {
                self.paused = !self.paused;
                Command::None
            }
            KeyCode::Char('u') => {
                self.converter = Some(ConverterState::default());
                Command::None
            }
            _ => Command::None,
        }
    }

    fn tick(&mut self) -> Command {
        self.drain_feed();
        Command::None
    }

    fn footer_hints(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("p", "Pause"),
            ("Ctrl+R", "Refresh"),
            ("u", "Converter"),
            ("Arrows", "Cursor"),
            ("y", "Copy"),
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

impl GasTrackerScreen {
    fn render_converter(&self, frame: &mut Frame<'_>, area: Rect) {
        let Some(state) = self.converter.as_ref() else {
            return;
        };
        let modal = centered_rect(area, 60, 40);
        frame.render_widget(Clear, modal);

        let body = format!(
            "Amount: {input}\n\
             From:   {from}   (←/→ to cycle)\n\
             To:     {to}   (↑/↓ to cycle)\n\
             {result}{error}\n\
             Enter to convert   Esc to close",
            input = if state.input.is_empty() {
                "_"
            } else {
                state.input.as_str()
            },
            from = state.from.label(),
            to = state.to.label(),
            result = state
                .result
                .as_deref()
                .map(|r| format!("\n=> {r} {}", state.to.label()))
                .unwrap_or_default(),
            error = state
                .error
                .as_deref()
                .map(|e| format!("\nerror: {e}"))
                .unwrap_or_default(),
        );

        frame.render_widget(
            Paragraph::new(body).wrap(Wrap { trim: false }).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Unit converter"),
            ),
            modal,
        );
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn format_trend(g: &GasSnapshot) -> String {
    if g.trend.is_empty() {
        return "(empty)".to_string();
    }
    g.trend
        .iter()
        .map(|v| v.value().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn histogram_bar(value: u128, max: u128) -> String {
    const WIDTH: usize = 20;
    if max == 0 {
        return "─".repeat(WIDTH);
    }
    let filled = ((value.saturating_mul(WIDTH as u128)) / max) as usize;
    let filled = filled.min(WIDTH);
    let mut bar = String::with_capacity(WIDTH);
    for _ in 0..filled {
        bar.push('█');
    }
    for _ in filled..WIDTH {
        bar.push('░');
    }
    bar
}

fn centered_rect(parent: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(parent)[1];

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical)[1]
}
