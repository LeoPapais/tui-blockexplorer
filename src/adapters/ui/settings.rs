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
    application::ports::{HealthLevel, HealthStatus},
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

/// Optional snapshot of provider health shown under the Overview
/// section. Fetched once on screen open; live polling is deferred.
/// See `plan/10-settings.md` section 12.5.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderHealthSnapshot {
    pub alchemy: Option<HealthStatus>,
    pub etherscan: Option<HealthStatus>,
}

pub struct SettingsScreen {
    snapshot: AppConfigSnapshot,
    health: ProviderHealthSnapshot,
}

impl SettingsScreen {
    #[must_use]
    pub fn new(snapshot: AppConfigSnapshot) -> Self {
        Self {
            snapshot,
            health: ProviderHealthSnapshot::default(),
        }
    }

    /// Attach a health snapshot to the screen. See
    /// `plan/10-settings.md` section 12.5.
    #[must_use]
    pub fn with_health(mut self, health: ProviderHealthSnapshot) -> Self {
        self.health = health;
        self
    }

    #[must_use]
    pub fn snapshot(&self) -> &AppConfigSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub fn health(&self) -> &ProviderHealthSnapshot {
        &self.health
    }

    fn render_providers(&self) -> String {
        fn line(label: &str, status: Option<&HealthStatus>) -> String {
            match status {
                Some(s) => {
                    let level = match s.status {
                        HealthLevel::Healthy => "healthy",
                        HealthLevel::Degraded => "degraded",
                        HealthLevel::Down => "down",
                    };
                    let suffix = s
                        .message
                        .as_deref()
                        .map(|m| format!(" ({m})"))
                        .unwrap_or_default();
                    format!("{label:<12} {level:<9} {latency}ms{suffix}", latency = s.latency_ms)
                }
                None => format!("{label:<12} (unknown)"),
            }
        }

        format!(
            "{}\n{}",
            line("alchemy", self.health.alchemy.as_ref()),
            line("etherscan-v2", self.health.etherscan.as_ref()),
        )
    }
}

impl Screen for SettingsScreen {
    fn title(&self) -> &str {
        "Settings"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(8),
                Constraint::Length(6),
            ])
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

        frame.render_widget(
            Paragraph::new(self.render_providers())
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title("Providers")),
            chunks[2],
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
