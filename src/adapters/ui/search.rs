//! Universal search modal.
//!
//! See `plan/2-search.md` section 10.3.
//!
//! The screen itself is purely synchronous: it owns the input string,
//! the current candidate list and the selected index. Resolution is
//! performed by an external task that pushes `SearchFeed` messages
//! into the screen; the screen answers back on keystroke by emitting a
//! query string through its sender half.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::screen::{Command, Screen},
    domain::ResolvedEntity,
};

/// Type of the callback used by `SearchScreen` to turn a resolved
/// candidate into a navigable detail screen. Lives on the screen so
/// the dispatcher does not need to know about any specific entity
/// kind.
pub type DetailFactory =
    Box<dyn Fn(ResolvedEntity) -> Box<dyn Screen> + Send + 'static>;

/// Message sent from the resolver task to the screen.
#[derive(Debug, Clone)]
pub struct SearchFeedUpdate {
    pub input: String,
    pub candidates: Vec<ResolvedEntity>,
}

/// Channel pair consumed by [`SearchScreen`].
pub struct SearchFeed {
    pub input_tx: UnboundedSender<String>,
    pub updates_rx: UnboundedReceiver<SearchFeedUpdate>,
}

/// Sender half exposed to the resolver task.
pub struct SearchFeedSender {
    pub updates_tx: UnboundedSender<SearchFeedUpdate>,
    pub input_rx: UnboundedReceiver<String>,
}

/// Build a new `(SearchFeed, SearchFeedSender)` pair.
#[must_use]
pub fn search_feed() -> (SearchFeed, SearchFeedSender) {
    let (input_tx, input_rx) = unbounded_channel();
    let (updates_tx, updates_rx) = unbounded_channel();
    (
        SearchFeed {
            input_tx,
            updates_rx,
        },
        SearchFeedSender {
            updates_tx,
            input_rx,
        },
    )
}

pub struct SearchScreen {
    input: String,
    candidates: Vec<ResolvedEntity>,
    selected: usize,
    feed: SearchFeed,
    on_detail: DetailFactory,
}

impl SearchScreen {
    /// Build a new `SearchScreen` that publishes query strings through
    /// `feed.input_tx` and accepts candidate lists through
    /// `feed.updates_rx`. `on_detail` is invoked when the user
    /// confirms a candidate with Enter.
    #[must_use]
    pub fn new(feed: SearchFeed, on_detail: DetailFactory) -> Self {
        Self {
            input: String::new(),
            candidates: Vec::new(),
            selected: 0,
            feed,
            on_detail,
        }
    }

    /// Current input string. Exposed for tests.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Current candidate list. Exposed for tests.
    #[must_use]
    pub fn candidates(&self) -> &[ResolvedEntity] {
        &self.candidates
    }

    /// Currently selected candidate, if any.
    #[must_use]
    pub fn selected(&self) -> Option<&ResolvedEntity> {
        self.candidates.get(self.selected)
    }

    /// Type one character into the search input and publish the new
    /// query. Exposed so tests and the key handler can share the
    /// logic.
    pub fn type_char(&mut self, c: char) {
        self.input.push(c);
        self.publish();
    }

    pub fn backspace(&mut self) {
        self.input.pop();
        self.publish();
    }

    pub fn set_candidates(&mut self, candidates: Vec<ResolvedEntity>) {
        self.selected = 0;
        self.candidates = candidates;
    }

    fn publish(&self) {
        let _ = self.feed.input_tx.send(self.input.clone());
    }

    fn drain_feed(&mut self) {
        while let Ok(update) = self.feed.updates_rx.try_recv() {
            // Ignore stale replies from earlier inputs.
            if update.input == self.input {
                self.set_candidates(update.candidates);
            }
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.candidates.is_empty() {
            return;
        }
        let len = self.candidates.len() as isize;
        let mut next = self.selected as isize + delta;
        if next < 0 {
            next = 0;
        } else if next >= len {
            next = len - 1;
        }
        self.selected = next as usize;
    }
}

impl Screen for SearchScreen {
    fn title(&self) -> &str {
        "Search"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(area);

        let header =
            Paragraph::new(format!("> {}_", self.input)).block(
                Block::default().borders(Borders::ALL).title("Search"),
            );
        frame.render_widget(header, chunks[0]);

        let items: Vec<ListItem<'_>> = self
            .candidates
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let marker = if i == self.selected { "> " } else { "  " };
                let text = format!("{marker}[{}] {}", e.kind_label(), render_entity(e));
                let style = if i == self.selected {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(text).style(style)
            })
            .collect();

        let list =
            List::new(items).block(Block::default().borders(Borders::ALL).title("Candidates"));
        frame.render_widget(list, chunks[1]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Esc => Command::Pop,
            KeyCode::Enter => match self.selected() {
                Some(ResolvedEntity::NotFound { .. }) => Command::None,
                Some(entity) => Command::Replace((self.on_detail)(entity.clone())),
                None => Command::None,
            },
            KeyCode::Backspace => {
                self.backspace();
                Command::None
            }
            KeyCode::Char(c) => {
                self.type_char(c);
                Command::None
            }
            KeyCode::Up => {
                self.move_selection(-1);
                Command::None
            }
            KeyCode::Down => {
                self.move_selection(1);
                Command::None
            }
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

fn render_entity(entity: &ResolvedEntity) -> String {
    match entity {
        ResolvedEntity::Block { number, hash } => {
            format!("#{}  {}", number.value(), short_hex(&hash.to_hex()))
        }
        ResolvedEntity::Tx { hash, block } => {
            let b = match block {
                Some(n) => format!(" (block {})", n.value()),
                None => String::new(),
            };
            format!("{}{}", short_hex(&hash.to_hex()), b)
        }
        ResolvedEntity::Address {
            address, ens_name, ..
        } => match ens_name {
            Some(name) => format!("{} ({name})", short_hex(&address.to_hex())),
            None => short_hex(&address.to_hex()),
        },
        ResolvedEntity::Token(m) => format!("{} - {}", m.symbol, m.name),
        ResolvedEntity::NotFound { reason } => reason.clone(),
    }
}

fn short_hex(s: &str) -> String {
    if s.len() <= 14 {
        return s.to_string();
    }
    format!("{}...{}", &s[..8], &s[s.len() - 4..])
}
