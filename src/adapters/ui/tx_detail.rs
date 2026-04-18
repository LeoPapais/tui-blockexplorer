//! Transaction Detail screen.
//!
//! Now surfaces Overview + Logs + Raw tabs. See
//! `plan/4-tx-detail.md` sections 12.3 (MVP) and 12.4.2 (this
//! expansion). Asset Changes and State Changes tabs arrive in commit
//! 3 of the plan-4 expansion.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block as RatBlock, Borders, Paragraph, Wrap},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::screen::{Command, Screen},
    application::{DecodedLog, DecodedMethod, TxView},
    domain::{Chain, TxHash, TxStatus},
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
    Raw,
}

impl TxTab {
    fn next(self) -> Self {
        match self {
            TxTab::Overview => TxTab::Logs,
            TxTab::Logs => TxTab::Raw,
            TxTab::Raw => TxTab::Overview,
        }
    }

    fn label(self) -> &'static str {
        match self {
            TxTab::Overview => "Overview",
            TxTab::Logs => "Logs",
            TxTab::Raw => "Raw",
        }
    }
}

pub struct TxDetailScreen {
    #[allow(dead_code)]
    chain: Chain,
    current: Option<TxView>,
    feed: TxFeed,
    active_tab: TxTab,
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

    fn drain_feed(&mut self) {
        while let Ok(view) = self.feed.updates_rx.try_recv() {
            self.current = Some(view);
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

        let header = match self.current.as_ref() {
            Some(view) => format!("Tx {}", short_hex(&view.tx.hash.to_hex())),
            None => "Tx (loading...)".to_string(),
        };
        frame.render_widget(
            Paragraph::new(header).block(
                RatBlock::default().borders(Borders::ALL).title("Transaction"),
            ),
            chunks[0],
        );

        let tabs = format!(
            "[ {over} ]  [ {logs} ]  [ {raw} ]",
            over = marker(self.active_tab, TxTab::Overview),
            logs = marker(self.active_tab, TxTab::Logs),
            raw = marker(self.active_tab, TxTab::Raw),
        );
        frame.render_widget(
            Paragraph::new(tabs).block(
                RatBlock::default().borders(Borders::ALL).title("Tabs"),
            ),
            chunks[1],
        );

        match (self.current.as_ref(), self.active_tab) {
            (None, _) => frame.render_widget(
                Paragraph::new("Loading...")
                    .block(RatBlock::default().borders(Borders::ALL).title("Overview")),
                chunks[2],
            ),
            (Some(view), TxTab::Overview) => frame.render_widget(
                Paragraph::new(overview_body(view))
                    .wrap(Wrap { trim: false })
                    .block(RatBlock::default().borders(Borders::ALL).title("Overview")),
                chunks[2],
            ),
            (Some(view), TxTab::Logs) => frame.render_widget(
                Paragraph::new(logs_body(&view.decoded_logs))
                    .wrap(Wrap { trim: false })
                    .block(RatBlock::default().borders(Borders::ALL).title("Logs")),
                chunks[2],
            ),
            (Some(view), TxTab::Raw) => frame.render_widget(
                Paragraph::new(view.tx.raw_json.clone())
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

fn overview_body(view: &TxView) -> String {
    let tx = &view.tx;
    let status = match &tx.status {
        TxStatus::Success => "success".to_string(),
        TxStatus::Failed { reason: Some(r) } => format!("failed - {r}"),
        TxStatus::Failed { reason: None } => "failed".to_string(),
        TxStatus::Pending => "pending".to_string(),
    };
    let to_line = match tx.to {
        Some(addr) => addr.to_hex(),
        None => "(contract creation)".to_string(),
    };
    let block = tx
        .block_number
        .map(|b| format!("#{}", b.value()))
        .unwrap_or_else(|| "(pending)".to_string());
    let idx = tx
        .tx_index
        .map(|i| i.to_string())
        .unwrap_or_else(|| "-".to_string());
    let gas_used = tx
        .gas_used
        .map(|g| g.to_string())
        .unwrap_or_else(|| "-".to_string());
    let fee = tx
        .fee_paid()
        .map(|w| format!("{} wei", w.value()))
        .unwrap_or_else(|| "(pending)".to_string());

    let method = method_line(view);
    format!(
        "Hash       {hash}\n\
Status     {status}\n\
Block      {block}\n\
Index      {idx}\n\
From       {from}\n\
To         {to}\n\
Value      {value} wei\n\
Gas used   {gas_used} / {gas_limit}  (price {gas_price} wei)\n\
Fee paid   {fee}\n\
Nonce      {nonce}\n\
Type       {ty}\n\
Method     {method}",
        hash = tx.hash.to_hex(),
        status = status,
        block = block,
        idx = idx,
        from = tx.from.to_hex(),
        to = to_line,
        value = tx.value.value(),
        gas_used = gas_used,
        gas_limit = tx.gas_limit,
        gas_price = tx.gas_price.value(),
        fee = fee,
        nonce = tx.nonce,
        ty = tx.tx_type.label(),
        method = method,
    )
}

fn method_line(view: &TxView) -> String {
    match view.decoded_method.as_ref() {
        Some(DecodedMethod { signature, source }) => {
            format!("{signature} ({tag})", tag = source.tag())
        }
        None => match view.tx.selector() {
            Some(sel) => format!(
                "0x{} (unknown)",
                hex::encode(sel),
            ),
            None if view.tx.input.is_empty() => "(empty)".to_string(),
            None => format!("0x{} (unknown)", hex::encode(&view.tx.input)),
        },
    }
}

fn logs_body(logs: &[DecodedLog]) -> String {
    if logs.is_empty() {
        return "No logs emitted.".to_string();
    }
    let mut out = String::new();
    for (idx, log) in logs.iter().enumerate() {
        let name = match log.signature.as_ref() {
            Some(sig) => format!("{signature} ({tag})", signature = sig.signature, tag = sig.source.tag()),
            None => match log.raw.topics.first() {
                Some(topic) => format!("0x{}... (unknown)", hex::encode(&topic[..4])),
                None => "(anonymous)".to_string(),
            },
        };
        out.push_str(&format!(
            "#{idx}  {addr}\n  event: {name}\n  data:  0x{data}\n",
            idx = idx,
            addr = log.raw.address.to_hex(),
            data = hex::encode(&log.raw.data),
        ));
        if !log.raw.topics.is_empty() {
            for (ti, topic) in log.raw.topics.iter().enumerate() {
                out.push_str(&format!("  t{ti}:    0x{}\n", hex::encode(topic)));
            }
        }
        out.push('\n');
    }
    out
}

fn short_hex(s: &str) -> String {
    if s.len() <= 14 {
        return s.to_string();
    }
    format!("{}...{}", &s[..8], &s[s.len() - 4..])
}
