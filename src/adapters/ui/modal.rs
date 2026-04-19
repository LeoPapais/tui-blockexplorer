//! Shared modal overlays.
//!
//! Modals live in the [`crate::adapters::ui::ScreenStack::modal`] slot
//! rather than on the back stack. The runtime renders the current
//! stack top first and then overlays the modal centered on top. The
//! help modal is the only concrete modal shipped today; confirm /
//! input variants are queued in `plan/12-screen-runtime.md` §8.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::adapters::ui::screen::{Command, Screen};

/// Context-sensitive help modal opened by pressing `?`.
///
/// The list of bindings is supplied by the caller: the global
/// dispatcher passes the active screen's binding hints plus the
/// global ones. Entries are rendered as `"key    description"`.
pub struct HelpModal {
    screen_title: String,
    entries: Vec<(String, String)>,
}

impl HelpModal {
    /// Build a new help modal for `screen_title` with the given
    /// binding entries (keys paired with human descriptions).
    #[must_use]
    pub fn new<S: Into<String>>(screen_title: S, entries: Vec<(String, String)>) -> Self {
        Self {
            screen_title: screen_title.into(),
            entries,
        }
    }

    /// Fully rendered body text. Exposed for snapshot / unit tests.
    #[must_use]
    pub fn body(&self) -> String {
        if self.entries.is_empty() {
            return "No bindings registered for this screen.\n\n\
                    Press Esc, Enter or ? to close."
                .to_string();
        }
        let max_key = self.entries.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
        let mut out = String::new();
        for (k, d) in &self.entries {
            out.push_str(&format!("{k:<width$}  {d}\n", width = max_key));
        }
        out.push_str("\nPress Esc, Enter or ? to close.");
        out
    }
}

impl Screen for HelpModal {
    fn title(&self) -> &str {
        "Help"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let modal = centered_rect(area, 60, 60);
        frame.render_widget(Clear, modal);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(modal);

        frame.render_widget(
            Paragraph::new(format!("Help — {}", self.screen_title))
                .style(Style::default().add_modifier(Modifier::BOLD))
                .block(Block::default().borders(Borders::ALL).title("plan/12 §7")),
            chunks[0],
        );
        frame.render_widget(
            Paragraph::new(self.body())
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title("Bindings")),
            chunks[1],
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?') | KeyCode::Char('q') => {
                Command::CloseModal
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

fn centered_rect(parent: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(parent)[1];

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical)[1]
}
