//! Universal search modal.
//!
//! See `plan/2-search.md` section 10.3 (integration) and §13.6 (line editor).
//!
//! The screen itself is purely synchronous: it owns the input string,
//! the current candidate list and the selected index. Resolution is
//! performed by an external task that pushes `SearchFeed` messages
//! into the screen; the screen answers back on keystroke by emitting a
//! query string through its sender half.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::{
        modal::{centered_rect, footer_rect},
        screen::{Command, Screen},
    },
    domain::ResolvedEntity,
};

/// Type of the callback used by `SearchScreen` to turn a resolved
/// candidate into a navigable detail screen. Lives on the screen so
/// the dispatcher does not need to know about any specific entity
/// kind.
pub type DetailFactory = Box<dyn Fn(ResolvedEntity) -> Box<dyn Screen> + Send + 'static>;

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
    /// Byte index into `input`, always on a UTF-8 character boundary.
    cursor: usize,
    /// Toggled on each UI tick so the caret blinks (~500 ms with 250 ms ticks).
    cursor_blink_on: bool,
    candidates: Vec<ResolvedEntity>,
    selected: usize,
    feed: SearchFeed,
    on_detail: DetailFactory,
}

fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    let len = s.len();
    if i > len {
        i = len;
    }
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn move_cursor_left(s: &str, cursor: usize) -> usize {
    let cursor = floor_char_boundary(s, cursor);
    if cursor == 0 {
        return 0;
    }
    s[..cursor]
        .chars()
        .next_back()
        .map(|c| cursor - c.len_utf8())
        .unwrap_or(0)
}

fn move_cursor_right(s: &str, cursor: usize) -> usize {
    let cursor = floor_char_boundary(s, cursor);
    if cursor >= s.len() {
        return cursor;
    }
    s[cursor..]
        .chars()
        .next()
        .map(|c| cursor + c.len_utf8())
        .unwrap_or(cursor)
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
            cursor: 0,
            cursor_blink_on: true,
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

    fn sync_cursor(&mut self) {
        self.cursor = floor_char_boundary(&self.input, self.cursor.min(self.input.len()));
    }

    /// Type one character into the search input at the cursor and publish the new
    /// query. Exposed so tests and the key handler can share the
    /// logic.
    pub fn type_char(&mut self, c: char) {
        self.sync_cursor();
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.publish();
    }

    pub fn backspace(&mut self) {
        self.sync_cursor();
        if self.cursor == 0 {
            return;
        }
        let prev = move_cursor_left(&self.input, self.cursor);
        self.input.replace_range(prev..self.cursor, "");
        self.cursor = prev;
        self.publish();
    }

    fn delete_forward(&mut self) {
        self.sync_cursor();
        if self.cursor >= self.input.len() {
            return;
        }
        let next = move_cursor_right(&self.input, self.cursor);
        self.input.replace_range(self.cursor..next, "");
        self.publish();
    }

    fn cursor_left(&mut self) {
        self.sync_cursor();
        self.cursor = move_cursor_left(&self.input, self.cursor);
    }

    fn cursor_right(&mut self) {
        self.sync_cursor();
        self.cursor = move_cursor_right(&self.input, self.cursor);
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

    fn input_line(&self) -> Line<'static> {
        let s = self.input.as_str();
        let cur = floor_char_boundary(s, self.cursor.min(s.len()));
        let cursor_style = |blink: bool| {
            if blink {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            }
        };

        let mut spans: Vec<Span<'static>> = vec![Span::raw("> ")];
        if cur >= s.len() {
            spans.push(Span::raw(s.to_string()));
            spans.push(Span::styled(" ", cursor_style(self.cursor_blink_on)));
            return Line::from(spans);
        }

        let ch = s[cur..]
            .chars()
            .next()
            .expect("cursor on boundary with cur < len");
        let after = cur + ch.len_utf8();
        spans.push(Span::raw(s[..cur].to_string()));
        spans.push(Span::styled(
            ch.to_string(),
            cursor_style(self.cursor_blink_on),
        ));
        spans.push(Span::raw(s[after..].to_string()));
        Line::from(spans)
    }
}

impl Screen for SearchScreen {
    fn title(&self) -> &str {
        "Search"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        // See `plan/2-search.md` §13: the search screen renders
        // as an overlay, not as a full-area screen. The input is
        // a 3-row vim-style command line pinned to the bottom;
        // the results sit in a centered floating block. Every
        // cell outside those two rectangles is left as the
        // backing screen painted it.
        let input_rect = footer_rect(area, 3);
        let mut results_rect = centered_rect(area, 60, 50);

        // Overlap guard for degenerate terminal dimensions: if
        // the centered block would crash into the footer, shrink
        // it upwards so it stays one row above the footer. When
        // that leaves zero rows, skip the results rectangle for
        // this frame so we never clip the input strip — the user
        // must always see where they are typing.
        let hide_results = if results_rect.bottom() > input_rect.top() {
            let new_bottom = input_rect.top().saturating_sub(1);
            if new_bottom <= results_rect.y {
                true
            } else {
                results_rect.height = new_bottom - results_rect.y;
                false
            }
        } else {
            false
        };

        if !hide_results {
            frame.render_widget(Clear, results_rect);
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
            frame.render_widget(list, results_rect);
        }

        frame.render_widget(Clear, input_rect);
        let header = Paragraph::new(self.input_line())
            .block(Block::default().borders(Borders::ALL).title("Search"));
        frame.render_widget(header, input_rect);
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
            KeyCode::Delete => {
                self.delete_forward();
                Command::None
            }
            KeyCode::Left => {
                self.cursor_left();
                Command::None
            }
            KeyCode::Right => {
                self.cursor_right();
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
        self.cursor_blink_on = !self.cursor_blink_on;
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
        ResolvedEntity::Contract { address } => {
            format!("{} (open as contract)", short_hex(&address.to_hex()))
        }
        ResolvedEntity::DelegatedEoa {
            address,
            delegated_to,
        } => format!(
            "{} (delegated to {})",
            short_hex(&address.to_hex()),
            short_hex(&delegated_to.to_hex()),
        ),
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
