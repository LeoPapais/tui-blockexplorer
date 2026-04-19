//! Modal screen that lists keybind conflicts at startup.
//!
//! Pushed on top of Home by `infra::run` when a user-supplied key
//! map fails validation. The modal is informational — the user
//! acknowledges by pressing Esc or Enter, which pops it off the
//! stack. The app then runs with the built-in bindings so the TUI
//! stays usable.
//!
//! See `plan/10-settings.md` section 12.4.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    adapters::ui::screen::{Command, Screen},
    domain::KeyBindConflictReport,
};

pub struct KeyBindConflictModal {
    report: KeyBindConflictReport,
}

impl KeyBindConflictModal {
    #[must_use]
    pub fn new(report: KeyBindConflictReport) -> Self {
        Self { report }
    }

    /// Access the underlying report. Tests downcast to the concrete
    /// modal and call this to assert on the surfaced conflicts.
    #[must_use]
    pub fn report(&self) -> &KeyBindConflictReport {
        &self.report
    }

    fn body(&self) -> String {
        format!("{}Press Esc or Enter to dismiss.", self.report)
    }
}

impl Screen for KeyBindConflictModal {
    fn title(&self) -> &str {
        "Keybind conflicts"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(area);

        frame.render_widget(
            Paragraph::new("Keybind conflicts")
                .style(Style::default().add_modifier(Modifier::BOLD))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("plan/10-settings.md §12.4"),
                ),
            chunks[0],
        );

        frame.render_widget(
            Paragraph::new(self.body())
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title("Conflicts")),
            chunks[1],
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => Command::Pop,
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
