//! Mempool screen (MVP: stub-driven stream, no filter modal).
//!
//! See `plan/5-mempool.md` section 11.2. The Alchemy WebSocket adapter
//! that turns this screen into a live view ships in a later slice; for
//! now the screen works end-to-end only when the composition root
//! injects a stub.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{
    adapters::ui::screen::{Command, Screen},
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
        }
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

    /// Update the client-side filter at runtime. Items already on
    /// screen that no longer match are pruned immediately.
    pub fn set_filter(&mut self, filter: PendingTxFilter) {
        self.filter = filter;
        self.items.retain(|tx| self.filter.matches(tx));
        if self.selected >= self.items.len() {
            self.selected = self.items.len().saturating_sub(1);
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
        if self.paused {
            return;
        }
        while let Ok(event) = self.rx.try_recv() {
            self.apply_event(event);
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
            .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Min(3)])
            .split(area);

        // Header
        let pause_badge = if self.paused { "  [paused]" } else { "" };
        let header = format!("Mempool  items: {}{pause_badge}", self.items.len());
        frame.render_widget(
            Paragraph::new(header).block(
                Block::default().borders(Borders::ALL).title("Mempool"),
            ),
            chunks[0],
        );

        // Filter summary
        let filter = match self.filter.from {
            Some(from) => format!("from: {}", from.to_hex()),
            None => "filter: none".to_string(),
        };
        frame.render_widget(
            Paragraph::new(filter).block(
                Block::default().borders(Borders::ALL).title("Filter"),
            ),
            chunks[1],
        );

        // Body
        if self.items.is_empty() {
            frame.render_widget(
                Paragraph::new("waiting for pending txs...").block(
                    Block::default().borders(Borders::ALL).title("Stream"),
                ),
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
            List::new(items).block(
                Block::default().borders(Borders::ALL).title("Stream"),
            ),
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
