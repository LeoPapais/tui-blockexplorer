//! Placeholder detail screen used until the real block/tx/address/
//! token detail screens land. Renders "{kind}: {identifier}".
//!
//! See `plan/2-search.md` section 10.3.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    adapters::ui::screen::{Command, Screen},
    domain::ResolvedEntity,
};

pub struct DetailPlaceholderScreen {
    entity: ResolvedEntity,
}

impl DetailPlaceholderScreen {
    #[must_use]
    pub fn new(entity: ResolvedEntity) -> Self {
        Self { entity }
    }

    #[must_use]
    pub fn entity(&self) -> &ResolvedEntity {
        &self.entity
    }

    fn body(&self) -> String {
        match &self.entity {
            ResolvedEntity::Block { number, hash } => {
                format!("Block #{}\nhash {}", number.value(), hash.to_hex())
            }
            ResolvedEntity::Tx { hash, block } => {
                let block_line = match block {
                    Some(b) => format!("\nin block #{}", b.value()),
                    None => String::new(),
                };
                format!("Tx {}{}", hash.to_hex(), block_line)
            }
            ResolvedEntity::Address {
                address,
                kind,
                ens_name,
            } => {
                let kind_label = match kind {
                    crate::domain::AddressKind::Eoa => "EOA",
                    crate::domain::AddressKind::Contract => "Contract",
                };
                let ens_line = ens_name
                    .as_ref()
                    .map(|n| format!("\nENS {n}"))
                    .unwrap_or_default();
                format!("Address {} ({}){}", address.to_hex(), kind_label, ens_line)
            }
            ResolvedEntity::Contract { address } => {
                format!("Contract {}", address.to_hex())
            }
            ResolvedEntity::Token(meta) => format!(
                "Token {} ({})\nname {}\ndecimals {}",
                meta.symbol,
                meta.address.to_hex(),
                meta.name,
                meta.decimals,
            ),
            ResolvedEntity::NotFound { reason } => reason.clone(),
        }
    }

    fn title_label(&self) -> &'static str {
        match self.entity {
            ResolvedEntity::Block { .. } => "Block",
            ResolvedEntity::Tx { .. } => "Transaction",
            ResolvedEntity::Address { .. } => "Address",
            ResolvedEntity::Contract { .. } => "Contract",
            ResolvedEntity::Token(_) => "Token",
            ResolvedEntity::NotFound { .. } => "Not found",
        }
    }
}

impl Screen for DetailPlaceholderScreen {
    fn title(&self) -> &str {
        self.title_label()
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title_label());
        let p = Paragraph::new(self.body())
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(p, area);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Char('q') => Command::Quit,
            KeyCode::Esc => Command::Pop,
            _ => Command::None,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
