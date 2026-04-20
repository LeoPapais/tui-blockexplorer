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

/// Build a rectangle centered on `parent` that occupies
/// `percent_x`% of the width and `percent_y`% of the height.
///
/// Reused by the Help modal (§7) and the Search overlay
/// (`plan/2-search.md` §13).
#[must_use]
pub(crate) fn centered_rect(parent: Rect, percent_x: u16, percent_y: u16) -> Rect {
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

/// Build a rectangle pinned to the bottom of `area` that spans
/// the full width and is `height` rows tall.
///
/// Used by the Search overlay (`plan/2-search.md` §13) for the
/// vim-style `:` command line. Saturating arithmetic keeps the
/// rectangle valid even when `height > area.height` (returns a
/// zero-row rect flush with the bottom).
#[must_use]
pub(crate) fn footer_rect(area: Rect, height: u16) -> Rect {
    let clamped = height.min(area.height);
    Rect {
        x: area.x,
        y: area.bottom().saturating_sub(clamped),
        width: area.width,
        height: clamped,
    }
}

#[cfg(test)]
mod tests {
    use super::{centered_rect, footer_rect};
    use ratatui::layout::Rect;

    #[test]
    fn footer_sits_at_the_bottom_of_area() {
        let area = Rect::new(0, 0, 120, 30);
        let rect = footer_rect(area, 3);
        assert_eq!(rect.x, 0);
        assert_eq!(rect.width, 120);
        assert_eq!(rect.height, 3);
        assert_eq!(rect.y, 27);
        assert_eq!(rect.bottom(), area.bottom());
    }

    #[test]
    fn footer_clamps_to_area_height() {
        let area = Rect::new(0, 0, 120, 2);
        let rect = footer_rect(area, 5);
        assert_eq!(rect.height, 2);
        assert_eq!(rect.y, 0);
        assert_eq!(rect.bottom(), area.bottom());
    }

    #[test]
    fn centered_rect_is_actually_centered() {
        let area = Rect::new(0, 0, 100, 100);
        let rect = centered_rect(area, 60, 50);
        assert_eq!(rect.width, 60);
        assert_eq!(rect.height, 50);
        assert_eq!(rect.x, 20);
        assert_eq!(rect.y, 25);
    }
}
