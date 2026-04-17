//! Transaction Detail screen (MVP: Overview + Raw tabs).
//!
//! See `plan/4-tx-detail.md` section 12.3. Logs / Internal / State
//! Changes / Asset Changes tabs are deferred until their backing
//! adapters land.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block as RatBlock, Borders, Paragraph, Wrap},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::screen::{Command, Screen},
    domain::{Chain, Transaction, TxHash, TxStatus},
};

pub struct TxFeed {
    pub input_tx: UnboundedSender<TxHash>,
    pub updates_rx: UnboundedReceiver<Transaction>,
}

pub struct TxFeedSender {
    pub updates_tx: UnboundedSender<Transaction>,
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
    Raw,
}

impl TxTab {
    fn next(self) -> Self {
        match self {
            TxTab::Overview => TxTab::Raw,
            TxTab::Raw => TxTab::Overview,
        }
    }

    fn label(self) -> &'static str {
        match self {
            TxTab::Overview => "Overview",
            TxTab::Raw => "Raw",
        }
    }
}

pub struct TxDetailScreen {
    #[allow(dead_code)]
    chain: Chain,
    current: Option<Transaction>,
    feed: TxFeed,
    active_tab: TxTab,
}

impl TxDetailScreen {
    /// Build a `TxDetailScreen` that will wait for the resolver task
    /// to deliver the transaction identified by `hash`.
    #[must_use]
    pub fn loading(chain: Chain, hash: TxHash, feed: TxFeed) -> Self {
        let _ = feed.input_tx.send(hash);
        Self {
            chain,
            current: None,
            feed,
            active_tab: TxTab::Overview,
        }
    }

    #[must_use]
    pub fn current(&self) -> Option<&Transaction> {
        self.current.as_ref()
    }

    #[must_use]
    pub fn active_tab(&self) -> TxTab {
        self.active_tab
    }

    fn drain_feed(&mut self) {
        while let Ok(tx) = self.feed.updates_rx.try_recv() {
            self.current = Some(tx);
        }
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

        // Header
        let header = match self.current.as_ref() {
            Some(tx) => format!("Tx {}", short_hex(&tx.hash.to_hex())),
            None => "Tx (loading...)".to_string(),
        };
        frame.render_widget(
            Paragraph::new(header).block(
                RatBlock::default().borders(Borders::ALL).title("Transaction"),
            ),
            chunks[0],
        );

        // Tab bar
        let tabs = format!(
            "[ {over} ]  [ {raw} ]",
            over = marker(self.active_tab, TxTab::Overview),
            raw = marker(self.active_tab, TxTab::Raw),
        );
        frame.render_widget(
            Paragraph::new(tabs).block(
                RatBlock::default().borders(Borders::ALL).title("Tabs"),
            ),
            chunks[1],
        );

        // Body
        match (self.current.as_ref(), self.active_tab) {
            (None, _) => frame.render_widget(
                Paragraph::new("Loading...")
                    .block(RatBlock::default().borders(Borders::ALL).title("Overview")),
                chunks[2],
            ),
            (Some(tx), TxTab::Overview) => frame.render_widget(
                Paragraph::new(overview_body(tx))
                    .wrap(Wrap { trim: false })
                    .block(RatBlock::default().borders(Borders::ALL).title("Overview")),
                chunks[2],
            ),
            (Some(tx), TxTab::Raw) => frame.render_widget(
                Paragraph::new(tx.raw_json.clone())
                    .wrap(Wrap { trim: false })
                    .block(RatBlock::default().borders(Borders::ALL).title("Raw")),
                chunks[2],
            ),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Char('q') => Command::Quit,
            KeyCode::Esc => Command::Pop,
            KeyCode::Tab | KeyCode::BackTab => {
                self.active_tab = self.active_tab.next();
                Command::None
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

fn marker(active: TxTab, tab: TxTab) -> String {
    if active == tab {
        format!("*{}*", tab.label())
    } else {
        tab.label().to_string()
    }
}

fn overview_body(tx: &Transaction) -> String {
    let status = match &tx.status {
        TxStatus::Success => "success".to_string(),
        TxStatus::Failed { reason: Some(r) } => format!("failed - {r}"),
        TxStatus::Failed { reason: None } => "failed".to_string(),
    };
    let to_line = match tx.to {
        Some(addr) => addr.to_hex(),
        None => "(contract creation)".to_string(),
    };
    let selector = if tx.input.len() >= 4 {
        format!(
            "0x{}{}{}{}",
            hex_byte(tx.input[0]),
            hex_byte(tx.input[1]),
            hex_byte(tx.input[2]),
            hex_byte(tx.input[3]),
        )
    } else if tx.input.is_empty() {
        "(empty)".to_string()
    } else {
        format!("0x{}", hex::encode(&tx.input))
    };
    format!(
        "Hash       {hash}\n\
Status     {status}\n\
Block      #{block}\n\
Index      {idx}\n\
From       {from}\n\
To         {to}\n\
Value      {value} wei\n\
Gas used   {gas_used} / {gas_limit}  (price {gas_price} wei)\n\
Fee paid   {fee} wei\n\
Nonce      {nonce}\n\
Type       {ty}\n\
Method     {selector} (raw selector)",
        hash = tx.hash.to_hex(),
        status = status,
        block = tx.block_number.value(),
        idx = tx.tx_index,
        from = tx.from.to_hex(),
        to = to_line,
        value = tx.value.value(),
        gas_used = tx.gas_used,
        gas_limit = tx.gas_limit,
        gas_price = tx.gas_price.value(),
        fee = tx.fee_paid().value(),
        nonce = tx.nonce,
        ty = tx.tx_type.label(),
        selector = selector,
    )
}

fn hex_byte(b: u8) -> String {
    format!("{b:02x}")
}

fn short_hex(s: &str) -> String {
    if s.len() <= 14 {
        return s.to_string();
    }
    format!("{}...{}", &s[..8], &s[s.len() - 4..])
}
