//! Home screen ratatui widget.
//!
//! Two levels of API live here:
//!
//! * [`render`] is the pure renderer consumed by both the real runtime
//!   and the snapshot tests.
//! * [`HomeScreen`] is the [`Screen`] implementation driven by the
//!   dispatcher; it holds the [`HomeViewModel`] and maps key events to
//!   [`Command`]s.
//!
//! See `plan/1-home.md` section 2 for the target layout and
//! `plan/12-screen-runtime.md` section 2.4 for the screen contract.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    adapters::ui::screen::{Command, Screen},
    application::{ConnectionStatus, HomeViewModel},
    domain::{BlockNumber, Chain, GasSnapshot, Gwei, NetworkStatus, Wei},
};

/// Render the Home screen into `frame` at `area`.
pub fn render(frame: &mut Frame<'_>, area: Rect, view: &HomeViewModel) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(10)])
        .split(area);

    render_header(frame, chunks[0], view);
    render_cards(frame, chunks[1], view);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, view: &HomeViewModel) {
    let badge = match view.connection {
        ConnectionStatus::Connected => String::new(),
        ConnectionStatus::Disconnected { reconnect_scheduled } => {
            if reconnect_scheduled {
                "  [disconnected, reconnecting]".to_string()
            } else {
                "  [disconnected]".to_string()
            }
        }
    };

    let text = format!("Chain: {}{}", view.chain.display_name(), badge);
    let para = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Home"))
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(para, area);
}

fn render_cards(frame: &mut Frame<'_>, area: Rect, view: &HomeViewModel) {
    let cards = Layout::default()
        .direction(if area.width >= 120 { Direction::Horizontal } else { Direction::Vertical })
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_network_card(frame, cards[0], view);
    render_gas_card(frame, cards[1], view);
}

fn render_network_card(frame: &mut Frame<'_>, area: Rect, view: &HomeViewModel) {
    let body = match view.network.as_ref() {
        Some(n) => format!(
            "Latest block   {}\nBlock time avg   {:.1}s\nBase fee   {} gwei",
            format_u64(n.latest_block.value()),
            n.block_time_avg_ms as f64 / 1000.0,
            n.base_fee.to_gwei().value(),
        ),
        None => "Loading network status...".to_string(),
    };
    let para = Paragraph::new(body)
        .alignment(Alignment::Left)
        .block(Block::default().borders(Borders::ALL).title("Network"));
    frame.render_widget(para, area);
}

fn render_gas_card(frame: &mut Frame<'_>, area: Rect, view: &HomeViewModel) {
    let body = match view.gas.as_ref() {
        Some(g) => format_gas(g),
        None => "Loading gas oracle...".to_string(),
    };
    let para = Paragraph::new(body)
        .alignment(Alignment::Left)
        .block(Block::default().borders(Borders::ALL).title("Gas Tracker"));
    frame.render_widget(para, area);
}

fn format_gas(g: &GasSnapshot) -> String {
    format!(
        "Slow    {} gwei\nAvg     {} gwei\nFast    {} gwei\nBase fee  {} gwei",
        g.slow.value(),
        g.average.value(),
        g.fast.value(),
        g.base_fee.value(),
    )
}

fn format_u64(n: u64) -> String {
    // Thousands separator using ASCII commas for terminal portability.
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*byte as char);
    }
    out
}

/// Screen-level wrapper around [`render`].
///
/// Holds the current [`HomeViewModel`] and routes key events to
/// [`Command`]s. This phase of the runtime serves the view model from
/// hardcoded data set at construction; phase 2 of the plan replaces the
/// constructor with one that owns a `HomeSession` and subscribes to a
/// background refresher task.
pub struct HomeScreen {
    view: HomeViewModel,
}

impl HomeScreen {
    /// Build a `HomeScreen` that renders whatever [`HomeViewModel`] it
    /// receives.
    #[must_use]
    pub fn new(view: HomeViewModel) -> Self {
        Self { view }
    }

    /// Placeholder view-model used by `cargo run -- --demo` until the
    /// Alchemy adapter lands in phase 2 (`plan/13-alchemy-adapter.md`).
    /// Numbers are plausible but frozen in time.
    #[must_use]
    pub fn with_demo_data() -> Self {
        let view = HomeViewModel {
            chain: Chain::Ethereum,
            network: Some(NetworkStatus {
                chain: Chain::Ethereum,
                latest_block: BlockNumber::new(21_345_678),
                base_fee: Wei::new(11_400_000_000),
                block_time_avg_ms: 12_100,
            }),
            gas: Some(GasSnapshot {
                chain: Chain::Ethereum,
                slow: Gwei::new(12),
                average: Gwei::new(14),
                fast: Gwei::new(18),
                base_fee: Gwei::new(11),
                trend: (0..20).map(|i| Gwei::new(11 + (i % 5))).collect(),
            }),
            connection: ConnectionStatus::Connected,
        };
        Self::new(view)
    }

    /// Expose the inner view-model. Useful for tests.
    #[must_use]
    pub fn view(&self) -> &HomeViewModel {
        &self.view
    }
}

impl Screen for HomeScreen {
    fn title(&self) -> &str {
        "Home"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        render(frame, area, &self.view);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Char('q') => Command::Quit,
            KeyCode::Esc => Command::Pop,
            _ => Command::None,
        }
    }
}
