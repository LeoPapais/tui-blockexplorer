//! Token Detail screen (Overview / Transfers / Chart tabs).
//!
//! The screen owns three independent channels pumped by
//! `src/infra/token_feed.rs`:
//!
//! - `updates_tx`    → `TokenOverview` (metadata + totalSupply).
//! - `price_tx`      → `PriceLookup` (spot-price status from the
//!   Prices API; see `plan/15-backlog.md` §3.4 for the three
//!   branches: `Available` / `Unsupported` / `Pending`).
//! - `transfers_tx`  → `TransferPage` (ERC-20 transfers filtered by
//!   the contract address).
//! - `history_tx`    → `PriceSeries` (historical price for the last
//!   selected window; the screen caches every window it has seen).
//!
//! Window switching (`1`/`2`/`3`) emits a `PriceWindow` request on
//! `window_req_tx`; the feed answers on `history_tx`.
//!
//! See `plan/8-token-detail.md` section 12.4.

use std::{any::Any, collections::HashMap};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, Block, Borders, Chart, Dataset, GraphType, List, ListItem, ListState, Paragraph,
        Tabs, Wrap,
    },
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::screen::{Command, Screen},
    domain::{
        Address, Chain, PriceLookup, PriceSeries, PriceWindow, TokenOverview, TokenPrice,
        TransferAsset, TransferEvent, TransferPage, TxHash,
    },
};

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

/// Channel half owned by the screen. The UI drains `*_rx` and emits
/// on `*_tx`.
pub struct TokenFeed {
    pub input_tx: UnboundedSender<Address>,
    pub window_req_tx: UnboundedSender<PriceWindow>,
    pub updates_rx: UnboundedReceiver<TokenOverview>,
    pub price_rx: UnboundedReceiver<PriceLookup>,
    pub transfers_rx: UnboundedReceiver<TransferPage>,
    pub history_rx: UnboundedReceiver<PriceSeries>,
}

/// Channel half owned by the background task. See
/// `src/infra/token_feed.rs`.
pub struct TokenFeedSender {
    pub updates_tx: UnboundedSender<TokenOverview>,
    pub price_tx: UnboundedSender<PriceLookup>,
    pub transfers_tx: UnboundedSender<TransferPage>,
    pub history_tx: UnboundedSender<PriceSeries>,
    pub input_rx: UnboundedReceiver<Address>,
    pub window_req_rx: UnboundedReceiver<PriceWindow>,
}

#[must_use]
pub fn token_feed() -> (TokenFeed, TokenFeedSender) {
    let (input_tx, input_rx) = unbounded_channel();
    let (window_req_tx, window_req_rx) = unbounded_channel();
    let (updates_tx, updates_rx) = unbounded_channel();
    let (price_tx, price_rx) = unbounded_channel();
    let (transfers_tx, transfers_rx) = unbounded_channel();
    let (history_tx, history_rx) = unbounded_channel();
    (
        TokenFeed {
            input_tx,
            window_req_tx,
            updates_rx,
            price_rx,
            transfers_rx,
            history_rx,
        },
        TokenFeedSender {
            updates_tx,
            price_tx,
            transfers_tx,
            history_tx,
            input_rx,
            window_req_rx,
        },
    )
}

/// Factory used by the Transfers tab to spawn a TxDetail screen
/// when the user hits Enter on a row.
pub type OpenTxFactory = Box<dyn Fn(TxHash) -> Box<dyn Screen> + Send + Sync>;

// ---------------------------------------------------------------------------
// Tabs / window
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenTab {
    Overview,
    Transfers,
    Chart,
}

impl TokenTab {
    pub const ALL: [TokenTab; 3] = [TokenTab::Overview, TokenTab::Transfers, TokenTab::Chart];

    /// Number of tabs exposed by the screen. Handy for BDD loops
    /// that cycle the `Tab` key until a target tab is reached.
    pub const ALL_COUNT: usize = Self::ALL.len();

    fn label(self) -> &'static str {
        match self {
            TokenTab::Overview => "Overview",
            TokenTab::Transfers => "Transfers",
            TokenTab::Chart => "Chart",
        }
    }
}

// ---------------------------------------------------------------------------
// Screen
// ---------------------------------------------------------------------------

pub struct TokenDetailScreen {
    #[allow(dead_code)]
    chain: Chain,
    address: Address,
    current: Option<TokenOverview>,
    price: PriceLookup,
    transfers: Option<TransferPage>,
    series: HashMap<PriceWindow, PriceSeries>,
    active_tab: TokenTab,
    active_window: PriceWindow,
    tx_list_state: ListState,
    feed: TokenFeed,
    open_tx: Option<OpenTxFactory>,
}

impl TokenDetailScreen {
    /// Build a screen in the loading state. Sends the initial
    /// address on `input_tx` so the feed starts fetching
    /// overview/price/transfers; also requests the default chart
    /// window (`D1`).
    #[must_use]
    pub fn loading(chain: Chain, address: Address, feed: TokenFeed) -> Self {
        Self::with_open_tx(chain, address, feed, None)
    }

    /// Loading constructor that wires Enter on the Transfers tab to
    /// open a TxDetail screen for the selected row.
    #[must_use]
    pub fn with_open_tx(
        chain: Chain,
        address: Address,
        feed: TokenFeed,
        open_tx: Option<OpenTxFactory>,
    ) -> Self {
        let _ = feed.input_tx.send(address);
        // The D1 chart window is fetched automatically by
        // `token_feed::spawn` as part of the address handling —
        // firing it explicitly here would race with that fetch
        // (the `tokio::select!` could previously pick the window
        // branch before `current_address` was populated and drop
        // the request).
        let mut tx_list_state = ListState::default();
        tx_list_state.select(Some(0));
        Self {
            chain,
            address,
            current: None,
            price: PriceLookup::Pending,
            transfers: None,
            series: HashMap::new(),
            active_tab: TokenTab::Overview,
            active_window: PriceWindow::D1,
            tx_list_state,
            feed,
            open_tx,
        }
    }

    // -- Accessors used by step definitions and unit tests ---------------

    #[must_use]
    pub fn current(&self) -> Option<&TokenOverview> {
        self.current.as_ref()
    }

    #[must_use]
    pub fn active_tab(&self) -> TokenTab {
        self.active_tab
    }

    #[must_use]
    pub fn active_window(&self) -> PriceWindow {
        self.active_window
    }

    #[must_use]
    pub fn transfers(&self) -> Option<&TransferPage> {
        self.transfers.as_ref()
    }

    #[must_use]
    pub fn price(&self) -> Option<&TokenPrice> {
        self.price.as_available()
    }

    /// Full [`PriceLookup`] status (available / unsupported /
    /// pending). Used by BDD steps that need to assert on the
    /// Unsupported branch surfaced by
    /// `plan/15-backlog.md` §3.4.
    #[must_use]
    pub fn price_lookup(&self) -> &PriceLookup {
        &self.price
    }

    /// Historical series for the currently selected window, if it
    /// has been received by the feed already.
    #[must_use]
    pub fn active_series(&self) -> Option<&PriceSeries> {
        self.series.get(&self.active_window)
    }

    #[must_use]
    pub fn selected_transfer(&self) -> Option<&TransferEvent> {
        self.transfers
            .as_ref()
            .and_then(|p| p.events.get(self.tx_list_state.selected().unwrap_or(0)))
    }

    // -- Internal helpers -----------------------------------------------

    fn drain_feed(&mut self) {
        while let Ok(ov) = self.feed.updates_rx.try_recv() {
            // Carry over any price already received so
            // `TokenOverview::market_cap` stays accurate regardless
            // of the order the metadata and price channels resolve
            // in.
            let mut ov = ov;
            if matches!(ov.price, PriceLookup::Pending) {
                ov.price = self.price.clone();
            }
            self.current = Some(ov);
        }
        while let Ok(lookup) = self.feed.price_rx.try_recv() {
            if let Some(ov) = self.current.as_mut() {
                ov.price = lookup.clone();
            }
            self.price = lookup;
        }
        while let Ok(page) = self.feed.transfers_rx.try_recv() {
            self.transfers = Some(page);
            self.clamp_tx_selection();
        }
        while let Ok(series) = self.feed.history_rx.try_recv() {
            self.series.insert(series.window, series);
        }
    }

    fn clamp_tx_selection(&mut self) {
        let len = self.transfers.as_ref().map(|p| p.events.len()).unwrap_or(0);
        if len == 0 {
            self.tx_list_state.select(None);
            return;
        }
        let current = self.tx_list_state.selected().unwrap_or(0);
        self.tx_list_state.select(Some(current.min(len - 1)));
    }

    fn select_delta(&mut self, delta: i32) {
        let len = self.transfers.as_ref().map(|p| p.events.len()).unwrap_or(0);
        if len == 0 {
            return;
        }
        let current = self.tx_list_state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1) as usize;
        self.tx_list_state.select(Some(next));
    }

    fn set_window(&mut self, window: PriceWindow) {
        if self.active_window == window {
            return;
        }
        self.active_window = window;
        if !self.series.contains_key(&window) {
            let _ = self.feed.window_req_tx.send(window);
        }
    }

    fn next_tab(&self) -> TokenTab {
        let idx = TokenTab::ALL
            .iter()
            .position(|t| *t == self.active_tab)
            .unwrap_or(0);
        TokenTab::ALL[(idx + 1) % TokenTab::ALL.len()]
    }
}

// ---------------------------------------------------------------------------
// Screen impl
// ---------------------------------------------------------------------------

impl Screen for TokenDetailScreen {
    fn title(&self) -> &str {
        "Token"
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

        frame.render_widget(
            Paragraph::new(header_line(self.current.as_ref(), self.address))
                .block(Block::default().borders(Borders::ALL).title("Token")),
            chunks[0],
        );

        let tabs: Vec<Line<'static>> = TokenTab::ALL
            .iter()
            .map(|t| Line::from(format!(" {} ", t.label())))
            .collect();
        let active_idx = TokenTab::ALL
            .iter()
            .position(|t| *t == self.active_tab)
            .unwrap_or(0);
        frame.render_widget(
            Tabs::new(tabs)
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

        match self.active_tab {
            TokenTab::Overview => self.render_overview(frame, chunks[2]),
            TokenTab::Transfers => self.render_transfers(frame, chunks[2]),
            TokenTab::Chart => self.render_chart(frame, chunks[2]),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        match (self.active_tab, key.code) {
            (_, KeyCode::Char('q')) => Command::Quit,
            (_, KeyCode::Esc) => Command::Pop,
            (_, KeyCode::Tab | KeyCode::BackTab) => {
                self.active_tab = self.next_tab();
                Command::None
            }

            // Chart window switch. The bindings are active from any
            // tab — if the user hits `2` while on Overview we still
            // queue up the request in the background.
            (_, KeyCode::Char('1')) => {
                self.set_window(PriceWindow::D1);
                Command::None
            }
            (_, KeyCode::Char('2')) => {
                self.set_window(PriceWindow::M1);
                Command::None
            }
            (_, KeyCode::Char('3')) => {
                self.set_window(PriceWindow::Y1);
                Command::None
            }

            // Transfers list nav.
            (TokenTab::Transfers, KeyCode::Up | KeyCode::Char('k')) => {
                self.select_delta(-1);
                Command::None
            }
            (TokenTab::Transfers, KeyCode::Down | KeyCode::Char('j')) => {
                self.select_delta(1);
                Command::None
            }
            (TokenTab::Transfers, KeyCode::PageUp) => {
                self.select_delta(-10);
                Command::None
            }
            (TokenTab::Transfers, KeyCode::PageDown) => {
                self.select_delta(10);
                Command::None
            }
            (TokenTab::Transfers, KeyCode::Enter) => {
                let hash = self.selected_transfer().map(|e| e.tx_hash);
                match (hash, self.open_tx.as_ref()) {
                    (Some(hash), Some(factory)) => Command::Push(factory(hash)),
                    _ => Command::None,
                }
            }
            _ => Command::None,
        }
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

// ---------------------------------------------------------------------------
// Render helpers
// ---------------------------------------------------------------------------

impl TokenDetailScreen {
    fn render_overview(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default().borders(Borders::ALL).title("Overview");
        let body = match self.current.as_ref() {
            None => "Loading...".to_string(),
            Some(ov) => {
                let price_cell = format_price_lookup(&self.price);
                let market_cap_cell = ov
                    .market_cap()
                    .map(format_market_cap)
                    .unwrap_or_else(|| "-".to_string());
                format!(
                    "Address       {addr}\n\
Symbol        {symbol}\n\
Name          {name}\n\
Decimals      {decimals}\n\
Total supply  {supply} (raw)\n\
\n\
Price         {price}\n\
Market cap    {mcap}\n\
\n\
[Tab] cycle tabs    [1] 1d  [2] 1m  [3] 1y    [Esc] back",
                    addr = ov.metadata.address.to_hex(),
                    symbol = ov.metadata.symbol,
                    name = ov.metadata.name,
                    decimals = ov.metadata.decimals,
                    supply = ov.total_supply,
                    price = price_cell,
                    mcap = market_cap_cell,
                )
            }
        };
        frame.render_widget(
            Paragraph::new(body).wrap(Wrap { trim: false }).block(block),
            area,
        );
    }

    fn render_transfers(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Transfers (ERC-20)");
        match self.transfers.as_ref() {
            None => frame.render_widget(Paragraph::new("Loading transfers...").block(block), area),
            Some(page) if page.events.is_empty() => frame.render_widget(
                Paragraph::new("No transfers found for this token yet.").block(block),
                area,
            ),
            Some(page) => {
                let items: Vec<ListItem> = page
                    .events
                    .iter()
                    .map(|e| ListItem::new(render_transfer_row(e)))
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
                    area,
                    &mut state,
                );
            }
        }
    }

    fn render_chart(&self, frame: &mut Frame<'_>, area: Rect) {
        let title = format!("Price chart ({label})", label = self.active_window.label());
        let block = Block::default().borders(Borders::ALL).title(title);

        let series = match self.series.get(&self.active_window) {
            None => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "Loading {label} price history...\n\
[1] 1d (1h)  [2] 1m (1d)  [3] 1y (1w)",
                        label = self.active_window.label(),
                    ))
                    .block(block),
                    area,
                );
                return;
            }
            Some(s) => s,
        };

        if series.points.is_empty() {
            frame.render_widget(
                Paragraph::new(format!(
                    "No price data for window {label}.\n\
[1] 1d (1h)  [2] 1m (1d)  [3] 1y (1w)",
                    label = self.active_window.label(),
                ))
                .block(block),
                area,
            );
            return;
        }

        let data: Vec<(f64, f64)> = series
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| (i as f64, p.value))
            .collect();
        let (lo, hi) = series.y_bounds().unwrap_or((0.0, 1.0));
        let y_min = if (hi - lo).abs() < f64::EPSILON {
            lo - 0.05 * lo.abs().max(1.0)
        } else {
            lo - (hi - lo) * 0.05
        };
        let y_max = if (hi - lo).abs() < f64::EPSILON {
            hi + 0.05 * hi.abs().max(1.0)
        } else {
            hi + (hi - lo) * 0.05
        };
        let x_max = (series.points.len().saturating_sub(1)) as f64;

        let datasets = vec![
            Dataset::default()
                .name("usd")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Cyan))
                .data(&data),
        ];

        let x_axis = Axis::default()
            .bounds([0.0, x_max.max(1.0)])
            .labels(vec![Span::raw("older"), Span::raw("now")])
            .style(Style::default().fg(Color::DarkGray));

        let y_axis = Axis::default()
            .bounds([y_min, y_max])
            .labels(vec![
                Span::raw(format_price(y_min)),
                Span::raw(format_price((y_min + y_max) / 2.0)),
                Span::raw(format_price(y_max)),
            ])
            .style(Style::default().fg(Color::DarkGray));

        let chart = Chart::new(datasets)
            .block(block)
            .x_axis(x_axis)
            .y_axis(y_axis);
        frame.render_widget(chart, area);
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

fn header_line(current: Option<&TokenOverview>, address: Address) -> String {
    match current {
        Some(ov) => format!(
            "Token {symbol}  \"{name}\"  {addr}",
            symbol = ov.metadata.symbol,
            name = ov.metadata.name,
            addr = ov.metadata.address.to_hex(),
        ),
        None => format!("Token {addr} (loading...)", addr = address.to_hex()),
    }
}

fn render_transfer_row(event: &TransferEvent) -> String {
    let from = short_addr(event.from.to_hex().as_str());
    let to = event
        .to
        .map(|a| short_addr(a.to_hex().as_str()))
        .unwrap_or_else(|| "(create)".to_string());
    let amount = format_transfer_amount(event);
    format!(
        "#{block:<10}  {from} -> {to}  {amount}",
        block = event.block_number.value(),
        from = from,
        to = to,
        amount = amount,
    )
}

fn format_transfer_amount(event: &TransferEvent) -> String {
    match &event.asset {
        TransferAsset::Erc20 {
            symbol, decimals, ..
        } => {
            let human = raw_to_human(event.value.value(), *decimals);
            format!("{human} {symbol}")
        }
        TransferAsset::Native { symbol } => {
            format!("{} {symbol}", event.value.value())
        }
        TransferAsset::Nft {
            symbol, token_id, ..
        } => format!("{symbol} #{token_id}"),
    }
}

fn raw_to_human(raw: u128, decimals: u8) -> String {
    if decimals == 0 {
        return raw.to_string();
    }
    let divisor = 10u128.pow(u32::from(decimals));
    let whole = raw / divisor;
    let frac = raw % divisor;
    // Trim trailing zeros from the fractional part for readability.
    let frac_str = format!("{:0width$}", frac, width = decimals as usize);
    let frac_trim = frac_str.trim_end_matches('0');
    if frac_trim.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{frac_trim}")
    }
}

/// Render a [`PriceLookup`] into the string shown in the Overview
/// price row. See `plan/15-backlog.md` §3.4 for the
/// "(not indexed by …)" spelling.
fn format_price_lookup(lookup: &PriceLookup) -> String {
    match lookup {
        PriceLookup::Available(p) => format_price(p.value),
        PriceLookup::Unsupported { provider } => format!("(not indexed by {provider})"),
        PriceLookup::Pending => "loading...".to_string(),
    }
}

fn format_price(value: f64) -> String {
    if !value.is_finite() {
        return "-".to_string();
    }
    if value == 0.0 {
        return "$0".to_string();
    }
    let abs = value.abs();
    if abs >= 1.0 {
        format!("${value:.4}")
    } else if abs >= 0.01 {
        format!("${value:.6}")
    } else {
        format!("${value:.8}")
    }
}

fn format_market_cap(value: f64) -> String {
    if !value.is_finite() || value <= 0.0 {
        return "-".to_string();
    }
    let rounded = value.round() as u128;
    let with_commas = group_thousands(rounded);
    format!("${with_commas}")
}

fn group_thousands(mut n: u128) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut parts = Vec::new();
    while n > 0 {
        parts.push(format!("{:03}", n % 1000));
        n /= 1000;
    }
    let first = parts.pop().unwrap();
    let first = first.trim_start_matches('0');
    let first = if first.is_empty() { "0" } else { first };
    let mut out = String::from(first);
    for chunk in parts.into_iter().rev() {
        out.push(',');
        out.push_str(&chunk);
    }
    out
}

fn short_addr(s: &str) -> String {
    if s.len() <= 12 {
        return s.to_string();
    }
    format!("{}...{}", &s[..6], &s[s.len() - 4..])
}

// ---------------------------------------------------------------------------
// Shared inline panel
// ---------------------------------------------------------------------------

/// Render the Token summary panel shared between the full
/// `TokenDetailScreen` and the inline Token tab on
/// `AddressDetailScreen`.
///
/// The panel splits `area` vertically: the top half lists the
/// overview fields (symbol, name, decimals, total supply, price,
/// market cap) and the bottom half draws a compact price chart of
/// `series` (braille line). When `series` is `None` or empty a
/// loading / empty state is drawn on the chart area instead.
///
/// `price` carries the full [`PriceLookup`] so the panel renders
/// `loading...`, `(not indexed by alchemy-prices)` or the actual
/// spot value depending on the provider's answer. See
/// `plan/6-address-detail.md` section 12.4.4 and
/// `plan/15-backlog.md` §3.4.
pub(crate) fn render_inline_token_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    overview: &TokenOverview,
    price: &PriceLookup,
    series: Option<&PriceSeries>,
    window: PriceWindow,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(3)])
        .split(area);

    let price_cell = format_price_lookup(price);
    // Compose a TokenOverview stitched with the price we have so
    // market_cap() can use it, without mutating the caller's copy.
    let market_cap_cell = {
        let synthesized = TokenOverview {
            metadata: overview.metadata.clone(),
            total_supply: overview.total_supply,
            price: price.clone(),
        };
        synthesized
            .market_cap()
            .map(format_market_cap)
            .unwrap_or_else(|| "-".to_string())
    };

    let supply_human = raw_to_human(overview.total_supply, overview.metadata.decimals);
    let body = format!(
        "Symbol        {symbol}\n\
Name          {name}\n\
Decimals      {decimals}\n\
Total supply  {supply_human} {symbol}  (raw {supply})\n\
Price         {price}\n\
Market cap    {mcap}\n\
\n\
[o] open full Token Detail",
        symbol = overview.metadata.symbol,
        name = overview.metadata.name,
        decimals = overview.metadata.decimals,
        supply_human = supply_human,
        supply = overview.total_supply,
        price = price_cell,
        mcap = market_cap_cell,
    );
    frame.render_widget(
        Paragraph::new(body)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Token")),
        chunks[0],
    );

    let chart_block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Price ({})", window.label()));
    match series {
        None => frame.render_widget(
            Paragraph::new(format!(
                "Loading {label} price history...",
                label = window.label(),
            ))
            .block(chart_block),
            chunks[1],
        ),
        Some(s) if s.points.is_empty() => frame.render_widget(
            Paragraph::new(format!(
                "No price data for window {label}.",
                label = window.label(),
            ))
            .block(chart_block),
            chunks[1],
        ),
        Some(s) => {
            let data: Vec<(f64, f64)> = s
                .points
                .iter()
                .enumerate()
                .map(|(i, p)| (i as f64, p.value))
                .collect();
            let (lo, hi) = s.y_bounds().unwrap_or((0.0, 1.0));
            let y_min = if (hi - lo).abs() < f64::EPSILON {
                lo - 0.05 * lo.abs().max(1.0)
            } else {
                lo - (hi - lo) * 0.05
            };
            let y_max = if (hi - lo).abs() < f64::EPSILON {
                hi + 0.05 * hi.abs().max(1.0)
            } else {
                hi + (hi - lo) * 0.05
            };
            let x_max = (s.points.len().saturating_sub(1)) as f64;
            let datasets = vec![
                Dataset::default()
                    .name("usd")
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(Color::Cyan))
                    .data(&data),
            ];
            let x_axis = Axis::default()
                .bounds([0.0, x_max.max(1.0)])
                .labels(vec![Span::raw("older"), Span::raw("now")])
                .style(Style::default().fg(Color::DarkGray));
            let y_axis = Axis::default()
                .bounds([y_min, y_max])
                .labels(vec![
                    Span::raw(format_price(y_min)),
                    Span::raw(format_price(y_max)),
                ])
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(
                Chart::new(datasets)
                    .block(chart_block)
                    .x_axis(x_axis)
                    .y_axis(y_axis),
                chunks[1],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_thousands_formats_values_with_us_grouping() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(7), "7");
        assert_eq!(group_thousands(1234), "1,234");
        assert_eq!(group_thousands(1_000_000), "1,000,000");
        assert_eq!(group_thousands(35_188_571), "35,188,571");
    }

    #[test]
    fn raw_to_human_trims_trailing_zeros() {
        assert_eq!(raw_to_human(1_000_000, 6), "1");
        assert_eq!(raw_to_human(1_234_500, 6), "1.2345");
        assert_eq!(raw_to_human(10, 6), "0.00001");
        assert_eq!(raw_to_human(0, 18), "0");
    }

    #[test]
    fn format_price_picks_precision_by_magnitude() {
        assert_eq!(format_price(1.0001), "$1.0001");
        assert!(format_price(0.5).starts_with("$0.500000"));
        assert!(format_price(0.0000001).starts_with("$0.0"));
    }
}
