//! Mempool screen (MVP: stub-driven stream, no filter modal).
//!
//! See `plan/5-mempool.md` section 11.2 for the base slice and
//! section 11.3 for the §8.6 follow-ups: the `update_filter` control
//! channel (§11.3.2) and the reconnecting badge fed by a status
//! channel (§11.3.3).

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, error::TryRecvError};

use crate::{
    adapters::ui::screen::{Command, Screen},
    application::ConnectionStatus,
    domain::{PendingTx, PendingTxEvent, PendingTxFilter, TxHash},
};

/// Callback used on Enter to open TxDetail for the selected row.
pub type OpenPendingTxFactory = Box<dyn Fn(TxHash) -> Box<dyn Screen> + Send + 'static>;

/// Sliding-window size for the pending-tx list. New additions evict
/// the oldest row once the window is full.
const WINDOW_SIZE: usize = 200;

pub struct MempoolScreen {
    rx: UnboundedReceiver<PendingTxEvent>,
    filter: PendingTxFilter,
    items: Vec<PendingTx>,
    selected: usize,
    paused: bool,
    open_tx_factory: OpenPendingTxFactory,
    /// Control channel that receives filter updates so the composition
    /// root can forward them to `PendingTxStreamPort::update_filter`.
    /// `None` means the screen runs without a server-side counterpart
    /// (demo mode, most BDD scenarios); filter changes still apply
    /// client-side.
    filter_control: Option<UnboundedSender<PendingTxFilter>>,
    /// Optional stream-connection status feed. `None` keeps the screen
    /// at [`ConnectionStatus::Connected`] for its entire lifetime.
    status_rx: Option<UnboundedReceiver<ConnectionStatus>>,
    stream_state: ConnectionStatus,
}

impl MempoolScreen {
    /// Build a Mempool screen subscribed to `rx`. The caller is
    /// responsible for keeping the sending half alive (via a stub or
    /// a live WS adapter).
    #[must_use]
    pub fn new(
        rx: UnboundedReceiver<PendingTxEvent>,
        filter: PendingTxFilter,
        open_tx_factory: OpenPendingTxFactory,
    ) -> Self {
        Self {
            rx,
            filter,
            items: Vec::new(),
            selected: 0,
            paused: false,
            open_tx_factory,
            filter_control: None,
            status_rx: None,
            stream_state: ConnectionStatus::Connected,
        }
    }

    /// Attach the control-channel sender used to forward filter
    /// changes to `PendingTxStreamPort::update_filter`. See
    /// `plan/5-mempool.md` §11.3.2.
    #[must_use]
    pub fn with_filter_control(mut self, control: UnboundedSender<PendingTxFilter>) -> Self {
        self.filter_control = Some(control);
        self
    }

    /// Attach a `ConnectionStatus` feed produced by the composition
    /// root. Drained on every tick; the latest status controls the
    /// reconnecting badge. See `plan/5-mempool.md` §11.3.3.
    #[must_use]
    pub fn with_status_feed(mut self, status_rx: UnboundedReceiver<ConnectionStatus>) -> Self {
        self.status_rx = Some(status_rx);
        self
    }

    /// Read-only access to the rendered items. Exposed for tests.
    #[must_use]
    pub fn items(&self) -> &[PendingTx] {
        &self.items
    }

    /// Whether the stream is currently paused.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Access the active client-side filter. Exposed for tests that
    /// assert on the default filter.
    #[must_use]
    pub fn filter(&self) -> &PendingTxFilter {
        &self.filter
    }

    /// Current connection status as surfaced by the header badge.
    #[must_use]
    pub fn stream_state(&self) -> &ConnectionStatus {
        &self.stream_state
    }

    /// Update the client-side filter at runtime. Items already on
    /// screen that no longer match are pruned immediately (see
    /// `plan/5-mempool.md` §11.3.2 for the rationale behind the
    /// prune-on-change semantic). When a control channel is attached
    /// (`with_filter_control`) the same filter is broadcast on that
    /// channel for the composition root to forward to
    /// `PendingTxStreamPort::update_filter`.
    pub fn set_filter(&mut self, filter: PendingTxFilter) {
        self.filter = filter;
        self.items.retain(|tx| self.filter.matches(tx));
        if self.selected >= self.items.len() {
            self.selected = self.items.len().saturating_sub(1);
        }
        if let Some(control) = self.filter_control.as_ref() {
            // Fire-and-forget: losing the server-side consumer is
            // acceptable because the client-side filter we just
            // applied already masks non-matching events.
            let _ = control.send(filter);
        }
    }

    fn apply_event(&mut self, event: PendingTxEvent) {
        match event {
            PendingTxEvent::Added(tx) => {
                if self.filter.matches(&tx) {
                    self.items.push(tx);
                    if self.items.len() > WINDOW_SIZE {
                        self.items.remove(0);
                        self.selected = self.selected.saturating_sub(1);
                    }
                }
            }
            PendingTxEvent::Removed(hash) => {
                if let Some(pos) = self.items.iter().position(|tx| tx.hash == hash) {
                    self.items.remove(pos);
                    if self.selected >= self.items.len() {
                        self.selected = self.items.len().saturating_sub(1);
                    }
                }
            }
        }
    }

    fn drain_feed(&mut self) {
        self.drain_status();
        if self.paused {
            return;
        }
        loop {
            match self.rx.try_recv() {
                Ok(event) => self.apply_event(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // The upstream sender vanished without publishing
                    // a status frame (stub/adapter crashed, socket
                    // closed, etc.). Reuse the same "reconnecting"
                    // state Home surfaces so the UX is consistent.
                    self.stream_state = ConnectionStatus::Disconnected {
                        reconnect_scheduled: true,
                    };
                    break;
                }
            }
        }
    }

    fn drain_status(&mut self) {
        let Some(rx) = self.status_rx.as_mut() else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(status) => self.stream_state = status,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // Status channel is gone. Drop it so we never poll
                    // a dead receiver; leave stream_state at its
                    // current value so a previously-seen Disconnected
                    // stays visible.
                    self.status_rx = None;
                    break;
                }
            }
        }
    }
}

impl Screen for MempoolScreen {
    fn title(&self) -> &str {
        "Mempool"
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

        // Header
        let pause_badge = if self.paused { "  [paused]" } else { "" };
        let status_badge = match self.stream_state {
            ConnectionStatus::Connected => "",
            ConnectionStatus::Disconnected {
                reconnect_scheduled: true,
            } => "  [reconnecting]",
            ConnectionStatus::Disconnected {
                reconnect_scheduled: false,
            } => "  [disconnected]",
        };
        let header = format!(
            "Mempool  items: {}{pause_badge}{status_badge}",
            self.items.len()
        );
        frame.render_widget(
            Paragraph::new(header).block(Block::default().borders(Borders::ALL).title("Mempool")),
            chunks[0],
        );

        // Filter summary
        let filter = match self.filter.from {
            Some(from) => format!("from: {}", from.to_hex()),
            None => "filter: none".to_string(),
        };
        frame.render_widget(
            Paragraph::new(filter).block(Block::default().borders(Borders::ALL).title("Filter")),
            chunks[1],
        );

        // Body
        if self.items.is_empty() {
            frame.render_widget(
                Paragraph::new("waiting for pending txs...")
                    .block(Block::default().borders(Borders::ALL).title("Stream")),
                chunks[2],
            );
            return;
        }

        let items: Vec<ListItem<'_>> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, tx)| {
                let marker = if i == self.selected { "> " } else { "  " };
                let style = if i == self.selected {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let to = tx
                    .to
                    .map(|a| short_hex(&a.to_hex()))
                    .unwrap_or_else(|| "(create)".to_string());
                ListItem::new(format!(
                    "{marker}{hash}  {from} -> {to}  {value} wei",
                    hash = short_hex(&tx.hash.to_hex()),
                    from = short_hex(&tx.from.to_hex()),
                    to = to,
                    value = tx.value.value(),
                ))
                .style(style)
            })
            .collect();

        frame.render_widget(
            List::new(items).block(Block::default().borders(Borders::ALL).title("Stream")),
            chunks[2],
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Char('q') => Command::Quit,
            KeyCode::Esc => Command::Pop,
            KeyCode::Char('p') => {
                self.paused = !self.paused;
                Command::None
            }
            KeyCode::Char('c') => {
                self.items.clear();
                self.selected = 0;
                Command::None
            }
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                Command::None
            }
            KeyCode::Down => {
                if !self.items.is_empty() {
                    self.selected = (self.selected + 1).min(self.items.len() - 1);
                }
                Command::None
            }
            KeyCode::Enter => {
                if let Some(tx) = self.items.get(self.selected) {
                    Command::Push((self.open_tx_factory)(tx.hash))
                } else {
                    Command::None
                }
            }
            _ => Command::None,
        }
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

fn short_hex(s: &str) -> String {
    if s.len() <= 14 {
        return s.to_string();
    }
    format!("{}...{}", &s[..8], &s[s.len() - 4..])
}
