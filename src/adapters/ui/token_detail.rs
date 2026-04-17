//! Token Detail screen (MVP: Overview tab only).
//!
//! See `plan/8-token-detail.md` section 12.3. Transfers and Chart
//! tabs show a deferred banner in the footer.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::screen::{Command, Screen},
    domain::{Address, Chain, TokenOverview},
};

pub struct TokenFeed {
    pub input_tx: UnboundedSender<Address>,
    pub updates_rx: UnboundedReceiver<TokenOverview>,
}

pub struct TokenFeedSender {
    pub updates_tx: UnboundedSender<TokenOverview>,
    pub input_rx: UnboundedReceiver<Address>,
}

#[must_use]
pub fn token_feed() -> (TokenFeed, TokenFeedSender) {
    let (input_tx, input_rx) = unbounded_channel();
    let (updates_tx, updates_rx) = unbounded_channel();
    (
        TokenFeed {
            input_tx,
            updates_rx,
        },
        TokenFeedSender {
            updates_tx,
            input_rx,
        },
    )
}

pub struct TokenDetailScreen {
    #[allow(dead_code)]
    chain: Chain,
    current: Option<TokenOverview>,
    feed: TokenFeed,
}

impl TokenDetailScreen {
    #[must_use]
    pub fn loading(chain: Chain, address: Address, feed: TokenFeed) -> Self {
        let _ = feed.input_tx.send(address);
        Self {
            chain,
            current: None,
            feed,
        }
    }

    #[must_use]
    pub fn current(&self) -> Option<&TokenOverview> {
        self.current.as_ref()
    }

    fn drain_feed(&mut self) {
        while let Ok(update) = self.feed.updates_rx.try_recv() {
            self.current = Some(update);
        }
    }
}

impl Screen for TokenDetailScreen {
    fn title(&self) -> &str {
        "Token"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(area);

        let header = match self.current.as_ref() {
            Some(ov) => format!(
                "Token {symbol}  \"{name}\"",
                symbol = ov.metadata.symbol,
                name = ov.metadata.name,
            ),
            None => "Token (loading...)".to_string(),
        };
        frame.render_widget(
            Paragraph::new(header).block(
                Block::default().borders(Borders::ALL).title("Token"),
            ),
            chunks[0],
        );

        let body = match self.current.as_ref() {
            Some(ov) => format!(
                "Address     {addr}\n\
Symbol      {symbol}\n\
Name        {name}\n\
Decimals    {decimals}\n\
Total supply  {supply} (raw)\n\
\n\
Deferred tabs: Transfers, Chart (price, market cap)\n\
(see plan/8-token-detail.md section 12.4)",
                addr = ov.metadata.address.to_hex(),
                symbol = ov.metadata.symbol,
                name = ov.metadata.name,
                decimals = ov.metadata.decimals,
                supply = ov.total_supply,
            ),
            None => "Loading...".to_string(),
        };
        frame.render_widget(
            Paragraph::new(body)
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title("Overview")),
            chunks[1],
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Char('q') => Command::Quit,
            KeyCode::Esc => Command::Pop,
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
