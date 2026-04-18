//! Gas Tracker screen (MVP).
//!
//! See `plan/9-gas-tracker.md` section 11.1.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::screen::{Command, Screen},
    domain::{Chain, GasSnapshot},
};

pub struct GasFeed {
    pub updates_rx: UnboundedReceiver<GasSnapshot>,
}

pub struct GasFeedSender {
    pub updates_tx: UnboundedSender<GasSnapshot>,
}

#[must_use]
pub fn gas_feed() -> (GasFeed, GasFeedSender) {
    let (tx, rx) = unbounded_channel();
    (GasFeed { updates_rx: rx }, GasFeedSender { updates_tx: tx })
}

pub struct GasTrackerScreen {
    #[allow(dead_code)]
    chain: Chain,
    current: Option<GasSnapshot>,
    feed: GasFeed,
}

impl GasTrackerScreen {
    /// Build a Gas Tracker screen subscribed to `feed`. If `initial`
    /// is set, the screen renders it straight away and replaces it
    /// with feed updates.
    #[must_use]
    pub fn new(chain: Chain, initial: Option<GasSnapshot>, feed: GasFeed) -> Self {
        Self {
            chain,
            current: initial,
            feed,
        }
    }

    #[must_use]
    pub fn current(&self) -> Option<&GasSnapshot> {
        self.current.as_ref()
    }

    fn drain_feed(&mut self) {
        while let Ok(update) = self.feed.updates_rx.try_recv() {
            self.current = Some(update);
        }
    }
}

impl Screen for GasTrackerScreen {
    fn title(&self) -> &str {
        "Gas Tracker"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(area);

        frame.render_widget(
            Paragraph::new("Gas Tracker")
                .block(Block::default().borders(Borders::ALL).title("Header")),
            chunks[0],
        );

        let body = match self.current.as_ref() {
            Some(g) => format!(
                "Slow     {slow} gwei\n\
Average  {avg} gwei\n\
Fast     {fast} gwei\n\
\n\
Base fee {base} gwei\n\
Trend    {trend}\n\
\n\
Press Esc to return, q to quit.",
                slow = g.slow.value(),
                avg = g.average.value(),
                fast = g.fast.value(),
                base = g.base_fee.value(),
                trend = format_trend(g),
            ),
            None => "Loading gas oracle...".to_string(),
        };
        frame.render_widget(
            Paragraph::new(body)
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title("Speeds")),
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

fn format_trend(g: &GasSnapshot) -> String {
    if g.trend.is_empty() {
        return "(empty)".to_string();
    }
    g.trend
        .iter()
        .map(|v| v.value().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}
