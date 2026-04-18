//! Block Detail screen.
//!
//! Renders the full [`Block`] entity across two tabs (Overview +
//! Transactions) and reacts to chronological navigation with `[` / `]`
//! through a `BlockFeed` channel pair. See `plan/3-block-detail.md`
//! section 11.3.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block as RatBlock, Borders, List, ListItem, Paragraph, Wrap},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::screen::{Command, Screen},
    domain::{Block, BlockId, BlockNumber, Chain, TxHash},
};

/// Channel half owned by the screen: feeds full blocks in and emits
/// block-id navigation requests.
pub struct BlockFeed {
    pub input_tx: UnboundedSender<BlockId>,
    pub updates_rx: UnboundedReceiver<Block>,
}

/// Channel half owned by the resolver task.
pub struct BlockFeedSender {
    pub updates_tx: UnboundedSender<Block>,
    pub input_rx: UnboundedReceiver<BlockId>,
}

/// Build a `(BlockFeed, BlockFeedSender)` channel pair.
#[must_use]
pub fn block_feed() -> (BlockFeed, BlockFeedSender) {
    let (input_tx, input_rx) = unbounded_channel();
    let (updates_tx, updates_rx) = unbounded_channel();
    (
        BlockFeed {
            input_tx,
            updates_rx,
        },
        BlockFeedSender {
            updates_tx,
            input_rx,
        },
    )
}

/// Callback used when the user presses Enter on a transaction row.
pub type OpenTxFactory = Box<dyn Fn(TxHash) -> Box<dyn Screen> + Send + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockTab {
    Overview,
    Transactions,
}

impl BlockTab {
    fn next(self) -> Self {
        match self {
            BlockTab::Overview => BlockTab::Transactions,
            BlockTab::Transactions => BlockTab::Overview,
        }
    }

    fn previous(self) -> Self {
        // Only two tabs so next == previous; kept as a distinct
        // method so callers read cleanly.
        self.next()
    }

    fn label(self) -> &'static str {
        match self {
            BlockTab::Overview => "Overview",
            BlockTab::Transactions => "Transactions",
        }
    }
}

pub struct BlockDetailScreen {
    #[allow(dead_code)]
    chain: Chain,
    current: Option<Block>,
    feed: BlockFeed,
    open_tx_factory: OpenTxFactory,
    active_tab: BlockTab,
    tx_selected: usize,
}

impl BlockDetailScreen {
    /// Build a screen in the loading state: sends an initial request
    /// for `id` on the feed and renders "Loading..." until the
    /// resolver task pushes the first block.
    #[must_use]
    pub fn loading(
        chain: Chain,
        id: BlockId,
        feed: BlockFeed,
        open_tx_factory: OpenTxFactory,
    ) -> Self {
        let _ = feed.input_tx.send(id);
        Self {
            chain,
            current: None,
            feed,
            open_tx_factory,
            active_tab: BlockTab::Overview,
            tx_selected: 0,
        }
    }

    /// Build a screen already primed with a block. Mostly useful in
    /// tests that want to bypass the feed.
    #[must_use]
    pub fn with_block(
        chain: Chain,
        block: Block,
        feed: BlockFeed,
        open_tx_factory: OpenTxFactory,
    ) -> Self {
        Self {
            chain,
            current: Some(block),
            feed,
            open_tx_factory,
            active_tab: BlockTab::Overview,
            tx_selected: 0,
        }
    }

    #[must_use]
    pub fn current(&self) -> Option<&Block> {
        self.current.as_ref()
    }

    #[must_use]
    pub fn active_tab(&self) -> BlockTab {
        self.active_tab
    }

    #[must_use]
    pub fn tx_selected(&self) -> usize {
        self.tx_selected
    }

    fn request_block(&self, id: BlockId) {
        let _ = self.feed.input_tx.send(id);
    }

    fn drain_feed(&mut self) {
        while let Ok(block) = self.feed.updates_rx.try_recv() {
            self.current = Some(block);
            self.tx_selected = 0;
        }
    }
}

impl Screen for BlockDetailScreen {
    fn title(&self) -> &str {
        "Block"
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
        let header = header_for(self.current.as_ref());
        frame.render_widget(
            Paragraph::new(header).block(RatBlock::default().borders(Borders::ALL).title("Block")),
            chunks[0],
        );

        // Tab bar
        let tabs = format!(
            "[ {overview} ]  [ {transactions} ]",
            overview = marker(self.active_tab, BlockTab::Overview),
            transactions = marker(self.active_tab, BlockTab::Transactions),
        );
        frame.render_widget(
            Paragraph::new(tabs).block(RatBlock::default().borders(Borders::ALL).title("Tabs")),
            chunks[1],
        );

        // Body
        match (self.current.as_ref(), self.active_tab) {
            (None, _) => {
                frame.render_widget(
                    Paragraph::new("Loading...")
                        .block(RatBlock::default().borders(Borders::ALL).title("Overview")),
                    chunks[2],
                );
            }
            (Some(block), BlockTab::Overview) => {
                let body = overview_body(block);
                frame.render_widget(
                    Paragraph::new(body)
                        .wrap(Wrap { trim: false })
                        .block(RatBlock::default().borders(Borders::ALL).title("Overview")),
                    chunks[2],
                );
            }
            (Some(block), BlockTab::Transactions) => {
                let items: Vec<ListItem<'_>> = block
                    .tx_hashes
                    .iter()
                    .enumerate()
                    .map(|(i, hash)| {
                        let marker = if i == self.tx_selected { "> " } else { "  " };
                        let style = if i == self.tx_selected {
                            Style::default().add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        ListItem::new(format!(
                            "{marker}{idx:>3}  {short}",
                            idx = i,
                            short = short_hex(&hash.to_hex()),
                        ))
                        .style(style)
                    })
                    .collect();
                frame.render_widget(
                    List::new(items).block(
                        RatBlock::default()
                            .borders(Borders::ALL)
                            .title(format!("Transactions ({})", block.tx_hashes.len())),
                    ),
                    chunks[2],
                );
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        // Navigation keys apply regardless of the active tab.
        match key.code {
            KeyCode::Char('q') => return Command::Quit,
            KeyCode::Esc => return Command::Pop,
            KeyCode::Char('[') => {
                if let Some(block) = self.current.as_ref() {
                    let prev = block.number.value().saturating_sub(1);
                    self.request_block(BlockId::Number(BlockNumber::new(prev)));
                }
                return Command::None;
            }
            KeyCode::Char(']') => {
                if let Some(block) = self.current.as_ref() {
                    let next = block.number.value().saturating_add(1);
                    self.request_block(BlockId::Number(BlockNumber::new(next)));
                }
                return Command::None;
            }
            KeyCode::Tab => {
                self.active_tab = self.active_tab.next();
                return Command::None;
            }
            KeyCode::BackTab => {
                self.active_tab = self.active_tab.previous();
                return Command::None;
            }
            KeyCode::Right => {
                self.active_tab = self.active_tab.next();
                return Command::None;
            }
            KeyCode::Left => {
                self.active_tab = self.active_tab.previous();
                return Command::None;
            }
            _ => {}
        }

        // Shift+Tab is reported as `KeyCode::Tab` + SHIFT on a few
        // terminals; handle that too.
        if key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT) {
            self.active_tab = self.active_tab.previous();
            return Command::None;
        }

        // Transactions-tab specific keys.
        if matches!(self.active_tab, BlockTab::Transactions)
            && let Some(block) = self.current.as_ref()
        {
            match key.code {
                KeyCode::Up => {
                    self.tx_selected = self.tx_selected.saturating_sub(1);
                }
                KeyCode::Down if !block.tx_hashes.is_empty() => {
                    self.tx_selected =
                        (self.tx_selected + 1).min(block.tx_hashes.len().saturating_sub(1));
                }
                KeyCode::Enter => {
                    if let Some(hash) = block.tx_hashes.get(self.tx_selected).copied() {
                        return Command::Push((self.open_tx_factory)(hash));
                    }
                }
                _ => {}
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

fn marker(active: BlockTab, tab: BlockTab) -> String {
    if active == tab {
        format!("*{}*", tab.label())
    } else {
        tab.label().to_string()
    }
}

fn header_for(current: Option<&Block>) -> String {
    match current {
        Some(b) => format!(
            "Block #{}  {}",
            format_u64(b.number.value()),
            short_hex(&b.hash.to_hex()),
        ),
        None => "Block (loading...)".to_string(),
    }
}

fn overview_body(b: &Block) -> String {
    let base_fee = b
        .base_fee
        .map(|w| format!("{} wei", w.value()))
        .unwrap_or_else(|| "-".to_string());
    format!(
        "Hash       {}\n\
Parent     {}\n\
Timestamp  {} (unix)\n\
Miner      {}\n\
Gas used   {} / {}\n\
Base fee   {}\n\
Size       {} bytes\n\
Txs        {}\n\
Extra      0x{}",
        b.hash.to_hex(),
        b.parent_hash.to_hex(),
        b.timestamp.seconds(),
        b.miner.to_hex(),
        format_u64(b.gas_used),
        format_u64(b.gas_limit),
        base_fee,
        format_u64(b.size),
        b.tx_hashes.len(),
        hex::encode(&b.extra_data),
    )
}

fn short_hex(s: &str) -> String {
    if s.len() <= 14 {
        return s.to_string();
    }
    format!("{}...{}", &s[..8], &s[s.len() - 4..])
}

fn format_u64(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*byte as char);
    }
    out
}
