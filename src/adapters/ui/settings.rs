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
    adapters::ui::{
        screen::{Command, Screen},
        theme::PalettePreset,
    },
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
    palette: PalettePreset,
}

impl SettingsScreen {
    #[must_use]
    pub fn new(snapshot: AppConfigSnapshot) -> Self {
        Self {
            snapshot,
            health: ProviderHealthSnapshot::default(),
            palette: PalettePreset::DarkDefault,
        }
    }

    /// Attach a health snapshot to the screen. See
    /// `plan/10-settings.md` section 12.5.
    #[must_use]
    pub fn with_health(mut self, health: ProviderHealthSnapshot) -> Self {
        self.health = health;
        self
    }

    /// Seed the active palette preset (typically from the user's
    /// saved preference). See `plan/10-settings.md` section 12.7.
    #[must_use]
    pub fn with_palette(mut self, preset: PalettePreset) -> Self {
        self.palette = preset;
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

    /// Currently-selected palette preset.
    #[must_use]
    pub fn palette(&self) -> PalettePreset {
        self.palette
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

    fn render_theme(&self) -> String {
        let mut out = String::new();
        for (idx, preset) in PalettePreset::all().iter().enumerate() {
            let marker = if *preset == self.palette { "*" } else { " " };
            out.push_str(&format!(
                "  {marker} [{idx}] {label}\n",
                idx = idx + 1,
                label = preset.label(),
            ));
        }
        out.push_str("Press 1-4 to switch palette (RGB picker deferred).");
        out
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
                Constraint::Length(7),
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

        frame.render_widget(
            Paragraph::new(self.render_theme())
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title("Theme")),
            chunks[3],
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Char('q') => Command::Quit,
            KeyCode::Esc => Command::Pop,
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let presets = PalettePreset::all();
                let idx = (c as usize).saturating_sub('1' as usize);
                if idx < presets.len() {
                    self.palette = presets[idx];
                }
                Command::None
            }
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
