//! Transaction Detail screen.
//!
//! Surfaces Overview / Logs / Asset Changes / State Changes / Raw
//! tabs. See `plan/4-tx-detail.md` sections 12.3 (MVP) and 12.4
//! (expansion).
//!
//! Each tab is scrollable: `Up/Down` (or `j`/`k`) moves by one line,
//! `PageUp`/`PageDown` by ten, `Home`/`End` jump to the extremes.
//! Switching tabs resets the scroll offset.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block as RatBlock, Borders, Paragraph, Tabs, Wrap},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::screen::{Command, Screen},
    application::{DecodedLog, DecodedMethod, LoadStatus, TxView},
    domain::{
        AddressStateDiff, AssetChange, AssetChangeKind, AssetKind, Chain, DiffChange,
        StateDiff, TxHash, TxStatus,
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

pub struct TxDetailScreen {
    #[allow(dead_code)]
    chain: Chain,
    current: Option<TxView>,
    feed: TxFeed,
    active_tab: TxTab,
    /// Vertical scroll offset, in wrapped lines, applied to the
    /// currently visible tab body. Resets whenever the active tab
    /// changes. Used by every tab so long outputs can be paged
    /// through without being truncated.
    scroll: u16,
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
            scroll: 0,
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

        let (title, body) = self.body_text();
        frame.render_widget(
            Paragraph::new(body)
                .wrap(Wrap { trim: false })
                .scroll((self.scroll, 0))
                .block(RatBlock::default().borders(Borders::ALL).title(title)),
            chunks[2],
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Char('q') => Command::Quit,
            KeyCode::Esc => Command::Pop,
            KeyCode::Tab | KeyCode::BackTab => {
                self.active_tab = self.active_tab.next();
                self.scroll = 0;
                Command::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll = self.scroll.saturating_sub(1);
                Command::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll = self.scroll.saturating_add(1);
                Command::None
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
                Command::None
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_add(10);
                Command::None
            }
            KeyCode::Home => {
                self.scroll = 0;
                Command::None
            }
            KeyCode::End => {
                self.scroll = u16::MAX;
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

impl TxDetailScreen {
    fn body_text(&self) -> (&'static str, String) {
        let Some(view) = self.current.as_ref() else {
            return ("Overview", "Loading...".to_string());
        };
        match self.active_tab {
            TxTab::Overview => ("Overview", overview_body(view)),
            TxTab::Logs => ("Logs", logs_body(&view.decoded_logs)),
            TxTab::AssetChanges => {
                ("Asset Changes", asset_changes_body(&view.asset_changes))
            }
            TxTab::StateChanges => {
                ("State Changes", state_changes_body(&view.state_diff))
            }
            TxTab::Raw => ("Raw", view.tx.raw_json.clone()),
        }
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
                    AssetKind::Erc20 { symbol, contract, .. } => {
                        format!("{symbol} @ {}", contract.to_hex())
                    }
                    AssetKind::Erc721 { symbol, contract, token_id } => {
                        format!("{symbol} #{token_id} @ {}", contract.to_hex())
                    }
                    AssetKind::Erc1155 { symbol, contract, token_id } => {
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
                    amount = change.amount.value(),
                ));
            }
            out
        }
    }
}

fn state_changes_body(status: &LoadStatus<StateDiff>) -> String {
    match status {
        LoadStatus::Pending => "Replaying transaction for state diff...".to_string(),
        LoadStatus::Unsupported => {
            "State-diff trace is unavailable on this chain / tier.".to_string()
        }
        LoadStatus::Failed(msg) => format!("Trace failed: {msg}"),
        LoadStatus::Loaded(diff) if diff.is_empty() => {
            "No state changes recorded.".to_string()
        }
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
