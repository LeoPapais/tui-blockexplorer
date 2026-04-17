//! Address Detail screen (MVP: Overview tab only).
//!
//! See `plan/6-address-detail.md` section 12.3. Transactions / Tokens
//! / Activity / Contract tabs wait for their respective adapters.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::screen::{Command, Screen},
    domain::{Address, AddressKind, AddressOverview, Chain},
};

/// Channel half owned by the screen.
pub struct AddressFeed {
    pub input_tx: UnboundedSender<Address>,
    pub updates_rx: UnboundedReceiver<AddressOverview>,
}

/// Channel half owned by the resolver task.
pub struct AddressFeedSender {
    pub updates_tx: UnboundedSender<AddressOverview>,
    pub input_rx: UnboundedReceiver<Address>,
}

#[must_use]
pub fn address_feed() -> (AddressFeed, AddressFeedSender) {
    let (input_tx, input_rx) = unbounded_channel();
    let (updates_tx, updates_rx) = unbounded_channel();
    (
        AddressFeed {
            input_tx,
            updates_rx,
        },
        AddressFeedSender {
            updates_tx,
            input_rx,
        },
    )
}

pub struct AddressDetailScreen {
    #[allow(dead_code)]
    chain: Chain,
    current: Option<AddressOverview>,
    feed: AddressFeed,
}

impl AddressDetailScreen {
    /// Build a screen in the loading state for `address`.
    #[must_use]
    pub fn loading(chain: Chain, address: Address, feed: AddressFeed) -> Self {
        let _ = feed.input_tx.send(address);
        Self {
            chain,
            current: None,
            feed,
        }
    }

    #[must_use]
    pub fn current(&self) -> Option<&AddressOverview> {
        self.current.as_ref()
    }

    fn drain_feed(&mut self) {
        while let Ok(update) = self.feed.updates_rx.try_recv() {
            self.current = Some(update);
        }
    }
}

impl Screen for AddressDetailScreen {
    fn title(&self) -> &str {
        "Address"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(area);

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

        match self.current.as_ref() {
            Some(ov) => {
                let kind = match ov.kind {
                    AddressKind::Eoa => "EOA",
                    AddressKind::Contract => "Contract",
                };
                let body = format!(
                    "Address     {addr}\n\
Kind        {kind}\n\
Balance     {balance} wei\n\
Nonce       {nonce}",
                    addr = ov.address.to_hex(),
                    kind = kind,
                    balance = ov.balance.value(),
                    nonce = ov.nonce,
                );
                frame.render_widget(
                    Paragraph::new(body)
                        .wrap(Wrap { trim: false })
                        .block(Block::default().borders(Borders::ALL).title("Overview")),
                    chunks[1],
                );
            }
            None => frame.render_widget(
                Paragraph::new("Loading...").block(
                    Block::default().borders(Borders::ALL).title("Overview"),
                ),
                chunks[1],
            ),
        }
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
