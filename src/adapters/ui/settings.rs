//! Settings screen (MVP: read-only snapshot of AppConfig).
//!
//! See `plan/10-settings.md` section 11.1.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    adapters::ui::screen::{Command, Screen},
    domain::Chain,
};

/// Lean view-model passed in at construction. Avoids pulling the
/// infra `AppConfig` type into the UI crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfigSnapshot {
    pub chain: Chain,
    pub alchemy_key_present: bool,
    pub config_path_hint: Option<String>,
}

pub struct SettingsScreen {
    snapshot: AppConfigSnapshot,
}

impl SettingsScreen {
    #[must_use]
    pub fn new(snapshot: AppConfigSnapshot) -> Self {
        Self { snapshot }
    }

    #[must_use]
    pub fn snapshot(&self) -> &AppConfigSnapshot {
        &self.snapshot
    }
}

impl Screen for SettingsScreen {
    fn title(&self) -> &str {
        "Settings"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(area);

        frame.render_widget(
            Paragraph::new("Settings")
                .block(Block::default().borders(Borders::ALL).title("Header")),
            chunks[0],
        );

        let alchemy = if self.snapshot.alchemy_key_present {
            "configured"
        } else {
            "not set (export ALCHEMY_API_KEY or edit config.toml)"
        };
        let path = self
            .snapshot
            .config_path_hint
            .as_deref()
            .unwrap_or("(no config file path resolved)");
        let body = format!(
            "Active chain   {chain}\n\
Alchemy key    {alchemy}\n\
Config file    {path}\n\
\n\
Editing credentials, chain list, theme and keybinds from inside\n\
the TUI is deferred; see plan/10-settings.md section 11.2. Press\n\
Esc to return to the previous screen, q to quit.",
            chain = self.snapshot.chain.display_name(),
            alchemy = alchemy,
            path = path,
        );
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
