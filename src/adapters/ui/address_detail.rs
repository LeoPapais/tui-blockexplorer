//! Address Detail screen.
//!
//! Renders Overview + Transactions tabs today; Tokens and Contract
//! tabs land in the follow-up commits of the plan-6 expansion
//! (`plan/6-address-detail.md` section 12.4).
//!
//! Overview summarises balance + kind + nonce + ENS hint. The
//! Transactions tab consumes a `TransferPage` fed by a background
//! task and exposes a selectable list; pressing Enter on a row
//! pushes a TxDetail screen through the supplied `OpenTxFactory`.

use std::any::Any;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::{
        screen::{Command, Screen},
        token_detail::render_inline_token_panel,
    },
    domain::{
        Address, AddressKind, AddressOverview, Chain, PriceSeries, PriceWindow, TokenHolding,
        TokenOverview, TokenPrice, TransferAsset, TransferEvent, TransferPage, TxHash,
    },
};

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

/// Channel half owned by the screen.
///
/// `token_overview_rx` / `token_price_rx` / `token_series_rx` feed
/// the inline Token tab that appears only when the address happens
/// to be an ERC-20 contract. See `plan/6-address-detail.md` section
/// 12.4.4.
pub struct AddressFeed {
    pub input_tx: UnboundedSender<Address>,
    pub updates_rx: UnboundedReceiver<AddressOverview>,
    pub transfers_rx: UnboundedReceiver<TransferPage>,
    pub portfolio_rx: UnboundedReceiver<Vec<TokenHolding>>,
    /// `Some(overview)` when the address is detected as an ERC-20
    /// contract; `None` means "probed and not a token" (the feed
    /// sends `None` explicitly to flip the tri-state).
    pub token_overview_rx: UnboundedReceiver<Option<TokenOverview>>,
    /// Spot price for the ERC-20 token. `Option<TokenPrice>` so the
    /// UI can distinguish "no data" from "still loading".
    pub token_price_rx: UnboundedReceiver<Option<TokenPrice>>,
    /// Historical series for the inline mini-chart (default window
    /// is `PriceWindow::D1`).
    pub token_series_rx: UnboundedReceiver<PriceSeries>,
}

/// Channel half owned by the background tasks.
pub struct AddressFeedSender {
    pub updates_tx: UnboundedSender<AddressOverview>,
    pub transfers_tx: UnboundedSender<TransferPage>,
    pub portfolio_tx: UnboundedSender<Vec<TokenHolding>>,
    pub token_overview_tx: UnboundedSender<Option<TokenOverview>>,
    pub token_price_tx: UnboundedSender<Option<TokenPrice>>,
    pub token_series_tx: UnboundedSender<PriceSeries>,
    pub input_rx: UnboundedReceiver<Address>,
}

#[must_use]
pub fn address_feed() -> (AddressFeed, AddressFeedSender) {
    let (input_tx, input_rx) = unbounded_channel();
    let (updates_tx, updates_rx) = unbounded_channel();
    let (transfers_tx, transfers_rx) = unbounded_channel();
    let (portfolio_tx, portfolio_rx) = unbounded_channel();
    let (token_overview_tx, token_overview_rx) = unbounded_channel();
    let (token_price_tx, token_price_rx) = unbounded_channel();
    let (token_series_tx, token_series_rx) = unbounded_channel();
    (
        AddressFeed {
            input_tx,
            updates_rx,
            transfers_rx,
            portfolio_rx,
            token_overview_rx,
            token_price_rx,
            token_series_rx,
        },
        AddressFeedSender {
            updates_tx,
            transfers_tx,
            portfolio_tx,
            token_overview_tx,
            token_price_tx,
            token_series_tx,
            input_rx,
        },
    )
}

/// Factory used by the Transactions tab to spawn a TxDetail screen
/// when the user presses Enter on a row.
pub type OpenTxFactory = Box<dyn Fn(TxHash) -> Box<dyn Screen> + Send + Sync>;

/// Factory used by the Tokens tab to spawn a TokenDetail screen
/// when the user presses Enter on a row.
pub type OpenTokenFactory = Box<dyn Fn(Address) -> Box<dyn Screen> + Send + Sync>;

/// Factory used by the Contract tab to spawn a ContractDetailScreen
/// when the user presses Enter. Only consulted when the loaded
/// `AddressOverview.kind` is `Contract`.
pub type OpenContractFactory = Box<dyn Fn(Address) -> Box<dyn Screen> + Send + Sync>;

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressTab {
    Overview,
    Transactions,
    Tokens,
    /// Only rendered when the address is detected as an ERC-20
    /// contract (see `token_overview` tri-state). Embeds the Token
    /// summary panel (symbol/supply/price/market cap + mini-chart)
    /// inline.
    Token,
    /// Only rendered when `AddressOverview.kind` is `Contract`.
    Contract,
}

impl AddressTab {
    fn label(self) -> &'static str {
        match self {
            AddressTab::Overview => "Overview",
            AddressTab::Transactions => "Transactions",
            AddressTab::Tokens => "Tokens",
            AddressTab::Token => "Token",
            AddressTab::Contract => "Contract",
        }
    }
}

/// Tri-state for the inline Token tab.
#[derive(Debug, Clone, PartialEq)]
enum TokenProbeState {
    /// The address is an EOA or the ERC-20 probe has not completed
    /// yet; the tab must stay hidden.
    Unknown,
    /// The address is a contract but the ERC-20 probe returned
    /// `Ok(None)`; tab stays hidden.
    NotToken,
    /// The probe succeeded: the tab is visible and renders this
    /// overview.
    IsToken(TokenOverview),
}

// ---------------------------------------------------------------------------
// Screen
// ---------------------------------------------------------------------------

pub struct AddressDetailScreen {
    #[allow(dead_code)]
    chain: Chain,
    address: Address,
    current: Option<AddressOverview>,
    transfers: Option<TransferPage>,
    holdings: Option<Vec<TokenHolding>>,
    /// Tri-state: see [`TokenProbeState`]. Controls visibility of
    /// the `AddressTab::Token` tab.
    token_probe: TokenProbeState,
    /// Spot price for the inline Token tab. `None` + `!token_price_missing`
    /// means "still loading"; `token_price_missing == true` means
    /// "probed and no data".
    token_price: Option<TokenPrice>,
    token_price_missing: bool,
    /// Historical series for the inline mini-chart (D1 only in MVP).
    token_series: Option<PriceSeries>,
    feed: AddressFeed,
    active_tab: AddressTab,
    scroll: u16,
    /// Upper bound on `scroll` for the currently rendered Overview
    /// body, refreshed on every render; used to stop `handle_key`
    /// from scrolling past the last visible row (plan 13.3).
    scroll_cap: std::cell::Cell<u16>,
    tx_list_state: ListState,
    token_list_state: ListState,
    open_tx: Option<OpenTxFactory>,
    open_token: Option<OpenTokenFactory>,
    open_contract: Option<OpenContractFactory>,
}

impl AddressDetailScreen {
    /// Build a screen in the loading state for `address`. The address
    /// is pushed through the input channel so the background task
    /// starts fetching immediately.
    #[must_use]
    pub fn loading(chain: Chain, address: Address, feed: AddressFeed) -> Self {
        Self::with_factories(chain, address, feed, None, None, None)
    }

    /// Same as [`loading`] but wires the Transactions tab to open a
    /// TxDetail screen when the user hits Enter on a row.
    #[must_use]
    pub fn with_open_tx(
        chain: Chain,
        address: Address,
        feed: AddressFeed,
        open_tx: Option<OpenTxFactory>,
    ) -> Self {
        Self::with_factories(chain, address, feed, open_tx, None, None)
    }

    /// Fully-wired constructor: Enter on the Transactions tab opens a
    /// TxDetail, Enter on the Tokens tab opens a TokenDetail, Enter
    /// on the Contract tab (only visible when kind == Contract) opens
    /// a ContractDetailScreen.
    #[must_use]
    pub fn with_factories(
        chain: Chain,
        address: Address,
        feed: AddressFeed,
        open_tx: Option<OpenTxFactory>,
        open_token: Option<OpenTokenFactory>,
        open_contract: Option<OpenContractFactory>,
    ) -> Self {
        let _ = feed.input_tx.send(address);
        let mut tx_list_state = ListState::default();
        tx_list_state.select(Some(0));
        let mut token_list_state = ListState::default();
        token_list_state.select(Some(0));
        Self {
            chain,
            address,
            current: None,
            transfers: None,
            holdings: None,
            token_probe: TokenProbeState::Unknown,
            token_price: None,
            token_price_missing: false,
            token_series: None,
            feed,
            active_tab: AddressTab::Overview,
            scroll: 0,
            scroll_cap: std::cell::Cell::new(0),
            tx_list_state,
            token_list_state,
            open_tx,
            open_token,
            open_contract,
        }
    }

    /// Tabs currently visible in the tab bar. Contract is included
    /// only when the loaded overview reports `AddressKind::Contract`.
    /// Token is included only when the ERC-20 probe resolved
    /// positively (see `plan/6-address-detail.md` section 12.4.4).
    fn visible_tabs(&self) -> Vec<AddressTab> {
        let mut tabs = vec![
            AddressTab::Overview,
            AddressTab::Transactions,
            AddressTab::Tokens,
        ];
        // Inline Token tab: only shown once we have a confirmed
        // ERC-20 TokenOverview in hand. Sits between Tokens
        // (portfolio) and Contract so all contract-specific tabs
        // cluster together.
        if matches!(self.token_probe, TokenProbeState::IsToken(_)) {
            tabs.push(AddressTab::Token);
        }
        if let Some(ov) = self.current.as_ref()
            && matches!(ov.kind, AddressKind::Contract)
        {
            tabs.push(AddressTab::Contract);
        }
        tabs
    }

    fn next_tab(&self) -> AddressTab {
        let tabs = self.visible_tabs();
        let current_idx = tabs
            .iter()
            .position(|&t| t == self.active_tab)
            .unwrap_or(0);
        tabs[(current_idx + 1) % tabs.len()]
    }

    fn prev_tab(&self) -> AddressTab {
        let tabs = self.visible_tabs();
        let current_idx = tabs
            .iter()
            .position(|&t| t == self.active_tab)
            .unwrap_or(0);
        tabs[(current_idx + tabs.len() - 1) % tabs.len()]
    }

    /// Public accessor so BDD scenarios can assert the tab bar
    /// composition (e.g. the Contract tab only appears for contract
    /// addresses).
    #[must_use]
    pub fn tabs(&self) -> Vec<AddressTab> {
        self.visible_tabs()
    }

    #[must_use]
    pub fn current(&self) -> Option<&AddressOverview> {
        self.current.as_ref()
    }

    #[must_use]
    pub fn transfers(&self) -> Option<&TransferPage> {
        self.transfers.as_ref()
    }

    #[must_use]
    pub fn holdings(&self) -> Option<&Vec<TokenHolding>> {
        self.holdings.as_ref()
    }

    #[must_use]
    pub fn active_tab(&self) -> AddressTab {
        self.active_tab
    }

    /// Inline Token overview recognised for this address, if any.
    /// Exposed so BDD scenarios can assert without downcasting.
    #[must_use]
    pub fn token_overview(&self) -> Option<&TokenOverview> {
        match &self.token_probe {
            TokenProbeState::IsToken(ov) => Some(ov),
            _ => None,
        }
    }

    /// Spot price for the inline Token tab, if loaded.
    #[must_use]
    pub fn token_price(&self) -> Option<&TokenPrice> {
        self.token_price.as_ref()
    }

    /// Historical series (D1 window) feeding the inline mini-chart.
    #[must_use]
    pub fn token_series(&self) -> Option<&PriceSeries> {
        self.token_series.as_ref()
    }

    /// Current selection index in the active list tab, clamped to the
    /// available rows. Returns 0 when the list is empty.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.active_list_state().selected().unwrap_or(0)
    }

    fn active_list_state(&self) -> &ListState {
        match self.active_tab {
            AddressTab::Tokens => &self.token_list_state,
            _ => &self.tx_list_state,
        }
    }

    fn active_list_state_mut(&mut self) -> &mut ListState {
        match self.active_tab {
            AddressTab::Tokens => &mut self.token_list_state,
            _ => &mut self.tx_list_state,
        }
    }

    fn active_list_len(&self) -> usize {
        match self.active_tab {
            AddressTab::Transactions => {
                self.transfers.as_ref().map(|p| p.events.len()).unwrap_or(0)
            }
            AddressTab::Tokens => self.holdings.as_ref().map(|h| h.len()).unwrap_or(0),
            _ => 0,
        }
    }

    fn drain_feed(&mut self) {
        while let Ok(update) = self.feed.updates_rx.try_recv() {
            self.current = Some(update);
        }
        while let Ok(page) = self.feed.transfers_rx.try_recv() {
            self.transfers = Some(page);
            self.clamp_tx_selection();
        }
        while let Ok(holdings) = self.feed.portfolio_rx.try_recv() {
            self.holdings = Some(holdings);
            self.clamp_token_selection();
        }
        while let Ok(opt_overview) = self.feed.token_overview_rx.try_recv() {
            self.token_probe = match opt_overview {
                Some(ov) => TokenProbeState::IsToken(ov),
                None => TokenProbeState::NotToken,
            };
        }
        while let Ok(opt_price) = self.feed.token_price_rx.try_recv() {
            match opt_price {
                Some(p) => {
                    self.token_price = Some(p);
                    self.token_price_missing = false;
                }
                None => {
                    self.token_price = None;
                    self.token_price_missing = true;
                }
            }
        }
        while let Ok(series) = self.feed.token_series_rx.try_recv() {
            self.token_series = Some(series);
        }
    }

    fn clamp_tx_selection(&mut self) {
        let len = self
            .transfers
            .as_ref()
            .map(|p| p.events.len())
            .unwrap_or(0);
        if len == 0 {
            self.tx_list_state.select(None);
            return;
        }
        let current = self.tx_list_state.selected().unwrap_or(0);
        self.tx_list_state.select(Some(current.min(len - 1)));
    }

    fn clamp_token_selection(&mut self) {
        let len = self.holdings.as_ref().map(|h| h.len()).unwrap_or(0);
        if len == 0 {
            self.token_list_state.select(None);
            return;
        }
        let current = self.token_list_state.selected().unwrap_or(0);
        self.token_list_state.select(Some(current.min(len - 1)));
    }

    fn select_delta(&mut self, delta: i32) {
        let len = self.active_list_len();
        if len == 0 {
            return;
        }
        let state = self.active_list_state_mut();
        let current = state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1);
        state.select(Some(next as usize));
    }
}

// ---------------------------------------------------------------------------
// Screen impl
// ---------------------------------------------------------------------------

impl Screen for AddressDetailScreen {
    fn title(&self) -> &str {
        "Address"
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
            Some(ov) => format!(
                "Address {addr}{ens}",
                addr = ov.address.to_hex(),
                ens = match ov.ens_name.as_deref() {
                    Some(n) => format!(" ({n})"),
                    None => String::new(),
                },
            ),
            None => "Address (loading...)".to_string(),
        };
        frame.render_widget(
            Paragraph::new(header).block(
                Block::default().borders(Borders::ALL).title("Address"),
            ),
            chunks[0],
        );

        // Tab bar — the Contract tab only shows up once the overview
        // loads and reports `AddressKind::Contract`.
        let tabs_visible = self.visible_tabs();
        let titles: Vec<Line<'static>> = tabs_visible
            .iter()
            .map(|t| Line::from(format!(" {} ", t.label())))
            .collect();
        let active_idx = tabs_visible
            .iter()
            .position(|&t| t == self.active_tab)
            .unwrap_or(0);
        frame.render_widget(
            Tabs::new(titles)
                .select(active_idx)
                .block(Block::default().borders(Borders::ALL).title("Tabs"))
                .divider(" ")
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::Indexed(238))
                        .fg(Color::White),
                ),
            chunks[1],
        );

        // Body
        match self.active_tab {
            AddressTab::Overview => {
                let body = overview_body(
                    self.current.as_ref(),
                    self.transfers.as_ref(),
                    self.holdings.as_ref(),
                );
                let content_lines = body.lines().count() as u16;
                let viewport = chunks[2].height.saturating_sub(2);
                let cap = content_lines.saturating_sub(viewport);
                self.scroll_cap.set(cap);
                let offset = self.scroll.min(cap);
                frame.render_widget(
                    Paragraph::new(body)
                        .wrap(Wrap { trim: false })
                        .scroll((offset, 0))
                        .block(Block::default().borders(Borders::ALL).title("Overview")),
                    chunks[2],
                );
            }
            AddressTab::Transactions => {
                let block = Block::default().borders(Borders::ALL).title("Transactions");
                match self.transfers.as_ref() {
                    None => frame.render_widget(
                        Paragraph::new("Loading transactions...").block(block),
                        chunks[2],
                    ),
                    Some(page) if page.events.is_empty() => frame.render_widget(
                        Paragraph::new("No transfers found for this address.").block(block),
                        chunks[2],
                    ),
                    Some(page) => {
                        let items: Vec<ListItem> = page
                            .events
                            .iter()
                            .map(|event| ListItem::new(render_transfer_row(event)))
                            .collect();
                        let mut state = self.tx_list_state;
                        frame.render_stateful_widget(
                            List::new(items)
                                .block(block)
                                .highlight_style(
                                    Style::default()
                                        .add_modifier(Modifier::BOLD)
                                        .bg(Color::Indexed(238)),
                                )
                                .highlight_symbol("> "),
                            chunks[2],
                            &mut state,
                        );
                    }
                }
            }
            AddressTab::Contract => {
                let body = format!(
                    "This address holds contract bytecode.\n\
\n\
Address   {addr}\n\
\n\
Press [Enter] to open the Contract Detail view (proxy hints,\n\
implementation resolution, source on Etherscan once wired).",
                    addr = self.address.to_hex(),
                );
                frame.render_widget(
                    Paragraph::new(body).wrap(Wrap { trim: false }).block(
                        Block::default().borders(Borders::ALL).title("Contract"),
                    ),
                    chunks[2],
                );
            }
            AddressTab::Tokens => {
                let block = Block::default().borders(Borders::ALL).title("Tokens");
                match self.holdings.as_ref() {
                    None => frame.render_widget(
                        Paragraph::new("Loading tokens...").block(block),
                        chunks[2],
                    ),
                    Some(holdings) if holdings.is_empty() => frame.render_widget(
                        Paragraph::new("No ERC-20 holdings found for this address.")
                            .block(block),
                        chunks[2],
                    ),
                    Some(holdings) => {
                        let items: Vec<ListItem> = holdings
                            .iter()
                            .map(|h| ListItem::new(render_token_row(h)))
                            .collect();
                        let mut state = self.token_list_state;
                        frame.render_stateful_widget(
                            List::new(items)
                                .block(block)
                                .highlight_style(
                                    Style::default()
                                        .add_modifier(Modifier::BOLD)
                                        .bg(Color::Indexed(238)),
                                )
                                .highlight_symbol("> "),
                            chunks[2],
                            &mut state,
                        );
                    }
                }
            }
            AddressTab::Token => {
                // `visible_tabs` guarantees we only reach this arm
                // when `token_probe == IsToken(..)`, so the unwrap
                // below is unreachable in practice.
                if let TokenProbeState::IsToken(ref ov) = self.token_probe {
                    render_inline_token_panel(
                        frame,
                        chunks[2],
                        ov,
                        self.token_price.as_ref(),
                        self.token_price_missing,
                        self.token_series.as_ref(),
                        PriceWindow::D1,
                    );
                }
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        let cmd = self.dispatch_key(key);
        // Clamp to the upper bound measured during the previous
        // render (plan 13.3), so `End` / `PageDown` stop exactly at
        // the last visible row instead of scrolling into the void.
        self.scroll = self.scroll.min(self.scroll_cap.get());
        cmd
    }

    fn tick(&mut self) -> Command {
        self.drain_feed();
        Command::None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl AddressDetailScreen {
    fn dispatch_key(&mut self, key: KeyEvent) -> Command {
        let is_back_tab = key.code == KeyCode::BackTab
            || (key.code == KeyCode::Tab
                && key.modifiers.contains(KeyModifiers::SHIFT));
        match (self.active_tab, key.code) {
            (_, KeyCode::Char('q')) => Command::Quit,
            (_, KeyCode::Esc) => Command::Pop,
            (_, _) if is_back_tab => {
                self.active_tab = self.prev_tab();
                self.scroll = 0;
                Command::None
            }
            (_, KeyCode::Tab) => {
                self.active_tab = self.next_tab();
                self.scroll = 0;
                Command::None
            }
            (AddressTab::Overview, KeyCode::Left) => {
                self.active_tab = self.prev_tab();
                self.scroll = 0;
                Command::None
            }
            (AddressTab::Overview, KeyCode::Right) => {
                self.active_tab = self.next_tab();
                self.scroll = 0;
                Command::None
            }

            // Overview scroll
            (AddressTab::Overview, KeyCode::Up | KeyCode::Char('k')) => {
                self.scroll = self.scroll.saturating_sub(1);
                Command::None
            }
            (AddressTab::Overview, KeyCode::Down | KeyCode::Char('j')) => {
                self.scroll = self.scroll.saturating_add(1);
                Command::None
            }
            (AddressTab::Overview, KeyCode::PageUp) => {
                self.scroll = self.scroll.saturating_sub(10);
                Command::None
            }
            (AddressTab::Overview, KeyCode::PageDown) => {
                self.scroll = self.scroll.saturating_add(10);
                Command::None
            }
            (AddressTab::Overview, KeyCode::Home) => {
                self.scroll = 0;
                Command::None
            }

            // Shared list navigation for Transactions / Tokens.
            (AddressTab::Transactions | AddressTab::Tokens, KeyCode::Up | KeyCode::Char('k')) => {
                self.select_delta(-1);
                Command::None
            }
            (
                AddressTab::Transactions | AddressTab::Tokens,
                KeyCode::Down | KeyCode::Char('j'),
            ) => {
                self.select_delta(1);
                Command::None
            }
            (AddressTab::Transactions | AddressTab::Tokens, KeyCode::PageUp) => {
                self.select_delta(-10);
                Command::None
            }
            (AddressTab::Transactions | AddressTab::Tokens, KeyCode::PageDown) => {
                self.select_delta(10);
                Command::None
            }
            (AddressTab::Transactions | AddressTab::Tokens, KeyCode::Home) => {
                self.active_list_state_mut().select(Some(0));
                Command::None
            }
            (AddressTab::Transactions | AddressTab::Tokens, KeyCode::End) => {
                let len = self.active_list_len();
                if len > 0 {
                    self.active_list_state_mut().select(Some(len - 1));
                }
                Command::None
            }

            (AddressTab::Transactions, KeyCode::Enter) => {
                let hash = self
                    .transfers
                    .as_ref()
                    .and_then(|p| p.events.get(self.selected()))
                    .map(|e| e.tx_hash);
                match (hash, self.open_tx.as_ref()) {
                    (Some(hash), Some(factory)) => Command::Push(factory(hash)),
                    _ => Command::None,
                }
            }
            (AddressTab::Tokens, KeyCode::Enter) => {
                let contract = self
                    .holdings
                    .as_ref()
                    .and_then(|h| h.get(self.selected()))
                    .map(|h| h.metadata.address);
                match (contract, self.open_token.as_ref()) {
                    (Some(addr), Some(factory)) => Command::Push(factory(addr)),
                    _ => Command::None,
                }
            }
            (AddressTab::Contract, KeyCode::Enter) => match self.open_contract.as_ref() {
                Some(factory) => Command::Push(factory(self.address)),
                None => Command::None,
            },
            // Token tab: `o` (or Enter) opens the full TokenDetail
            // screen for the same address. The inline panel is
            // read-only — no list navigation to handle.
            (AddressTab::Token, KeyCode::Char('o') | KeyCode::Enter) => {
                match self.open_token.as_ref() {
                    Some(factory) => Command::Push(factory(self.address)),
                    None => Command::None,
                }
            }
            _ => Command::None,
        }
    }
}

// ---------------------------------------------------------------------------
// Render helpers
// ---------------------------------------------------------------------------

fn overview_body(
    overview: Option<&AddressOverview>,
    transfers: Option<&TransferPage>,
    holdings: Option<&Vec<TokenHolding>>,
) -> String {
    match overview {
        None => "Loading...".to_string(),
        Some(ov) => {
            let kind = match ov.kind {
                AddressKind::Eoa => "EOA",
                AddressKind::Contract => "Contract",
            };
            let tx_count = transfers
                .map(|p| p.events.len())
                .map(|n| n.to_string())
                .unwrap_or_else(|| "loading...".to_string());
            let token_count = holdings
                .map(|h| h.len())
                .map(|n| n.to_string())
                .unwrap_or_else(|| "loading...".to_string());
            format!(
                "Address       {addr}\n\
Kind          {kind}\n\
Balance       {balance} wei\n\
Nonce         {nonce}\n\
\n\
Txs loaded    {tx_count}\n\
Tokens loaded {token_count}\n\
\n\
[Tab] cycle tabs    [Enter] open selection    [Esc] back",
                addr = ov.address.to_hex(),
                kind = kind,
                balance = ov.balance.value(),
                nonce = ov.nonce,
                tx_count = tx_count,
                token_count = token_count,
            )
        }
    }
}

fn render_token_row(h: &TokenHolding) -> String {
    format!(
        "{symbol:<10}  {balance:<20}  d={decimals:<2}  {contract}",
        symbol = h.metadata.symbol,
        balance = h.balance.value(),
        decimals = h.metadata.decimals,
        contract = h.metadata.address.to_hex(),
    )
}

fn render_transfer_row(event: &TransferEvent) -> String {
    let cat = event.category.label();
    let from = short_addr(event.from.to_hex().as_str());
    let to = event
        .to
        .map(|a| short_addr(a.to_hex().as_str()))
        .unwrap_or_else(|| "(create)".to_string());
    let amount = format_amount(event);
    let block = event.block_number.value();
    format!(
        "[{cat:>7}] #{block:<10}  {from} -> {to}  {amount}",
        cat = cat,
        block = block,
        from = from,
        to = to,
        amount = amount,
    )
}

fn format_amount(event: &TransferEvent) -> String {
    let symbol = event.asset.symbol();
    match &event.asset {
        TransferAsset::Native { .. } => {
            format!("{value} wei {symbol}", value = event.value.value())
        }
        TransferAsset::Erc20 { decimals, .. } => {
            format!(
                "{v} {symbol}  (raw 0x{raw:x}, d={decimals})",
                v = event.value.value(),
                raw = event.value.value(),
                decimals = decimals,
            )
        }
        TransferAsset::Nft { token_id, kind, .. } => {
            let kind_label = match kind {
                crate::domain::NftKind::Erc721 => "721",
                crate::domain::NftKind::Erc1155 => "1155",
            };
            format!("{symbol} #{token_id} ({kind_label})")
        }
    }
}

fn short_addr(s: &str) -> String {
    if s.len() <= 12 {
        return s.to_string();
    }
    format!("{}...{}", &s[..6], &s[s.len() - 4..])
}
