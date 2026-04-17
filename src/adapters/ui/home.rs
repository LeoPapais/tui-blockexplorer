//! Home screen ratatui widget.
//!
//! Pure renderer: consumes a [`HomeViewModel`] and paints a frame. Input
//! handling and async data fetching live elsewhere (the UI dispatcher owns
//! keybinds, the application layer owns data).
//!
//! See `plan/1-home.md` section 2 for the target layout and
//! `plan/1-home.md` section 11.6 for the place of this widget in the
//! pipeline.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    application::{ConnectionStatus, HomeViewModel},
    domain::GasSnapshot,
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
