//! Contract Detail screen (MVP: Overview tab only, with EIP-1967
//! proxy detection).
//!
//! See `plan/7-contract-detail.md` section 12.3. Source / ABI / Read /
//! Events / Storage tabs remain deferred behind the footer note.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::screen::{Command, Screen},
    domain::{Address, Chain, ContractOverview},
};

pub struct ContractFeed {
    pub input_tx: UnboundedSender<Address>,
    pub updates_rx: UnboundedReceiver<ContractOverview>,
}

pub struct ContractFeedSender {
    pub updates_tx: UnboundedSender<ContractOverview>,
    pub input_rx: UnboundedReceiver<Address>,
}

#[must_use]
pub fn contract_feed() -> (ContractFeed, ContractFeedSender) {
    let (input_tx, input_rx) = unbounded_channel();
    let (updates_tx, updates_rx) = unbounded_channel();
    (
        ContractFeed {
            input_tx,
            updates_rx,
        },
        ContractFeedSender {
            updates_tx,
            input_rx,
        },
    )
}

pub struct ContractDetailScreen {
    #[allow(dead_code)]
    chain: Chain,
    current: Option<ContractOverview>,
    feed: ContractFeed,
}

impl ContractDetailScreen {
    /// Build a screen in the loading state: emits one request on the
    /// feed and renders "Loading..." until the resolver task replies.
    #[must_use]
    pub fn loading(chain: Chain, address: Address, feed: ContractFeed) -> Self {
        let _ = feed.input_tx.send(address);
        Self {
            chain,
            current: None,
            feed,
        }
    }

    #[must_use]
    pub fn current(&self) -> Option<&ContractOverview> {
        self.current.as_ref()
    }

    fn drain_feed(&mut self) {
        while let Ok(update) = self.feed.updates_rx.try_recv() {
            self.current = Some(update);
        }
    }
}

impl Screen for ContractDetailScreen {
    fn title(&self) -> &str {
        "Contract"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(area);

        let header = match self.current.as_ref() {
            Some(ov) => format!("Contract {}", ov.account.address.to_hex()),
            None => "Contract (loading...)".to_string(),
        };
        frame.render_widget(
            Paragraph::new(header).block(
                Block::default().borders(Borders::ALL).title("Contract"),
            ),
            chunks[0],
        );

        let body = match self.current.as_ref() {
            Some(ov) => {
                let balance = ov.account.balance.value();
                let nonce = ov.account.nonce;
                let proxy_line = match ov.proxy {
                    Some(info) => format!(
                        "Proxy      {kind} -> {impl_addr}",
                        kind = info.kind.label(),
                        impl_addr = info.implementation.to_hex(),
                    ),
                    None => "Proxy      not detected".to_string(),
                };
                format!(
                    "Address    {addr}\n\
Balance    {balance} wei\n\
Nonce      {nonce}\n\
{proxy_line}\n\
\n\
Deferred tabs: Source, ABI, Read, Events, Storage\n\
(see plan/7-contract-detail.md section 12.4)",
                    addr = ov.account.address.to_hex(),
                )
            }
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
