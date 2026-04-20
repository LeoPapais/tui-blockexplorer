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
    adapters::ui::{
        field_cursor::{CursorDir, CursorServices, FieldCursor, FieldEntry},
        screen::{Command, Screen},
    },
    domain::{Block, BlockId, BlockNumber, Chain, NavigableValue, TxHash},
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
    /// Post-Shanghai withdrawals plus a placeholder for the Beacon
    /// blob sidecars (still deferred, see
    /// `plan/3-block-detail.md` §13).
    BlobsAndWithdrawals,
}

impl BlockTab {
    fn next(self) -> Self {
        match self {
            BlockTab::Overview => BlockTab::Transactions,
            BlockTab::Transactions => BlockTab::BlobsAndWithdrawals,
            BlockTab::BlobsAndWithdrawals => BlockTab::Overview,
        }
    }

    fn previous(self) -> Self {
        match self {
            BlockTab::Overview => BlockTab::BlobsAndWithdrawals,
            BlockTab::Transactions => BlockTab::Overview,
            BlockTab::BlobsAndWithdrawals => BlockTab::Transactions,
        }
    }

    fn label(self) -> &'static str {
        match self {
            BlockTab::Overview => "Overview",
            BlockTab::Transactions => "Transactions",
            BlockTab::BlobsAndWithdrawals => "Blobs / Withdrawals",
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
    /// Last value produced by the `y` / `Y` bindings before the
    /// cursor takes over. Retained so the existing test sink
    /// (`last_copied_value()`) keeps working and the migration to
    /// [`CursorServices`] stays incremental. See
    /// `plan/17-navigable-values.md` §7.
    last_copied_value: Option<String>,
    cursor: FieldCursor,
    cursor_services: Option<CursorServices>,
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
            last_copied_value: None,
            cursor: FieldCursor::new(),
            cursor_services: None,
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
            last_copied_value: None,
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

    /// Navigable fields exposed on the Overview tab. Order matches
    /// the on-screen reading order: number, hash, parent hash, miner.
    #[must_use]
    pub fn navigable_fields(&self) -> Vec<FieldEntry> {
        let Some(b) = self.current.as_ref() else {
            return Vec::new();
        };
        let mut out = vec![
            FieldEntry::new("block_number", NavigableValue::BlockNumber(b.number)),
            FieldEntry::new("block_hash", NavigableValue::BlockHash(b.hash)),
            FieldEntry::new("parent_hash", NavigableValue::BlockHash(b.parent_hash)),
            FieldEntry::new("miner", NavigableValue::Address(b.miner)),
        ];
        if let Some(signer) = b.extra_signer {
            out.push(FieldEntry::new("signer", NavigableValue::Address(signer)));
        }
        out
    }

    /// Current cursor state. Exposed for tests.
    #[must_use]
    pub const fn cursor(&self) -> &FieldCursor {
        &self.cursor
    }

    /// Latest value produced by the `y` / `Y` clipboard bindings.
    /// Mirrors `TxDetailScreen::last_copied_value`. Returns `None`
    /// before the user triggers a copy.
    #[must_use]
    pub fn last_copied_value(&self) -> Option<&str> {
        self.last_copied_value.as_deref()
    }

    /// Copy the identifier most relevant to the active tab:
    ///
    /// - Overview: the block hash.
    /// - Transactions: the selected transaction hash.
    ///
    /// No-op while the block is still loading; covered by
    /// `plan/3-block-detail.md` §12.1.
    fn copy_active_identifier(&mut self) {
        let Some(block) = self.current.as_ref() else {
            return;
        };
        let value = match self.active_tab {
            BlockTab::Overview | BlockTab::BlobsAndWithdrawals => {
                NavigableValue::BlockHash(block.hash)
            }
            BlockTab::Transactions => match block.tx_hashes.get(self.tx_selected) {
                Some(hash) => NavigableValue::TxHash(*hash),
                None => return,
            },
        };
        self.last_copied_value = Some(value.copy_text());
        if let Some(services) = self.cursor_services.as_ref() {
            services.copy(&value);
        }
    }

    fn copy_block_number(&mut self) {
        let Some(block) = self.current.as_ref() else {
            return;
        };
        let value = NavigableValue::BlockNumber(block.number);
        self.last_copied_value = Some(value.copy_text());
        if let Some(services) = self.cursor_services.as_ref() {
            services.copy(&value);
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
            "[ {overview} ]  [ {transactions} ]  [ {blobs} ]",
            overview = marker(self.active_tab, BlockTab::Overview),
            transactions = marker(self.active_tab, BlockTab::Transactions),
            blobs = marker(self.active_tab, BlockTab::BlobsAndWithdrawals),
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
            (Some(block), BlockTab::BlobsAndWithdrawals) => {
                let body = blobs_and_withdrawals_body(block);
                frame.render_widget(
                    Paragraph::new(body).wrap(Wrap { trim: false }).block(
                        RatBlock::default()
                            .borders(Borders::ALL)
                            .title("Blobs / Withdrawals"),
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
            KeyCode::Esc if self.cursor.is_active() => {
                self.cursor.deactivate();
                return Command::None;
            }
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
            // Left/Right are now owned by the field cursor on the
            // Overview tab (see the §7 dispatch below). The legacy
            // "Left/Right switches tabs" behaviour only kicks in
            // when the active tab is not Overview — Transactions and
            // Blobs/Withdrawals still flip tabs with arrows, matching
            // the pre-cursor contract.
            KeyCode::Right if !matches!(self.active_tab, BlockTab::Overview) => {
                self.active_tab = self.active_tab.next();
                return Command::None;
            }
            KeyCode::Left if !matches!(self.active_tab, BlockTab::Overview) => {
                self.active_tab = self.active_tab.previous();
                return Command::None;
            }
            // `y` copies the identifier relevant to the active tab;
            // `Y` always copies the block number. See plan/3 §12.1.
            // Once the cursor takes over (see the §7 dispatch below)
            // `y` copies the field under the cursor instead.
            KeyCode::Char('y') if !self.cursor.is_active() => {
                self.copy_active_identifier();
                return Command::None;
            }
            KeyCode::Char('Y') => {
                self.copy_block_number();
                return Command::None;
            }
            _ => {}
        }

        // Cursor is only in scope on the Overview tab so list
        // navigation in the Transactions tab keeps its existing
        // meaning.
        if matches!(self.active_tab, BlockTab::Overview) {
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
                        self.last_copied_value = Some(entry.value.copy_text());
                        services.copy(&entry.value);
                    } else if let Some(entry) = self.cursor.current(&fields) {
                        // No services: keep the legacy sink in sync
                        // so the previous tests still have something
                        // to assert on.
                        self.last_copied_value = Some(entry.value.copy_text());
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
    // Polygon PoS and other Bor-style chains leave `miner` at the zero
    // address and publish the real validator in `extraData`. We render
    // the recovered signer immediately below the miner row so the user
    // sees both the raw header value and the recovered author.
    let signer_row = match b.extra_signer {
        Some(addr) => format!("\nSigner     {}", addr.to_hex()),
        None => String::new(),
    };
    format!(
        "Hash       {}\n\
Parent     {}\n\
Timestamp  {} (unix)\n\
Miner      {}{signer_row}\n\
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

fn blobs_and_withdrawals_body(b: &Block) -> String {
    let mut out = String::new();
    out.push_str("Withdrawals (");
    out.push_str(&b.withdrawals.len().to_string());
    out.push_str(")\n");
    if b.withdrawals.is_empty() {
        out.push_str("  (none on this block)\n");
    } else {
        for w in &b.withdrawals {
            out.push_str(&format!(
                "  #{idx:>3}  validator {val:<7}  {addr}  {amount} gwei\n",
                idx = w.index,
                val = w.validator_index,
                addr = short_hex(&w.address.to_hex()),
                amount = format_u64(w.amount_gwei),
            ));
        }
    }
    out.push_str("\nBlobs\n");
    out.push_str("  Beacon blob_sidecars not wired yet — see plan/3-block-detail.md §13.\n");
    out
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
