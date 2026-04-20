//! Line editor behaviour on the universal search modal.
//!
//! See `plan/2-search.md` §13.6.

use blockexplorer_tui::{
    adapters::ui::{Command, Screen, SearchScreen, search_feed},
    domain::ResolvedEntity,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn it_inserts_in_the_middle_when_cursor_moves_left() {
    let (feed, _) = search_feed();
    let detail: blockexplorer_tui::adapters::ui::DetailFactory =
        Box::new(|_: ResolvedEntity| unreachable!("not used"));
    let mut s = SearchScreen::new(feed, detail);

    s.handle_key(key(KeyCode::Char('a')));
    s.handle_key(key(KeyCode::Char('b')));
    s.handle_key(key(KeyCode::Char('c')));
    assert_eq!(s.input(), "abc");

    s.handle_key(key(KeyCode::Left));
    s.handle_key(key(KeyCode::Left));
    s.handle_key(key(KeyCode::Char('x')));

    assert_eq!(s.input(), "axbc");
}

#[test]
fn it_backspaces_before_cursor_not_only_at_end() {
    let (feed, _) = search_feed();
    let detail: blockexplorer_tui::adapters::ui::DetailFactory =
        Box::new(|_: ResolvedEntity| unreachable!("not used"));
    let mut s = SearchScreen::new(feed, detail);

    s.handle_key(key(KeyCode::Char('a')));
    s.handle_key(key(KeyCode::Char('b')));
    s.handle_key(key(KeyCode::Left));
    s.handle_key(key(KeyCode::Backspace));

    assert_eq!(s.input(), "b");
}

#[test]
fn it_deletes_forward_at_cursor() {
    let (feed, _) = search_feed();
    let detail: blockexplorer_tui::adapters::ui::DetailFactory =
        Box::new(|_: ResolvedEntity| unreachable!("not used"));
    let mut s = SearchScreen::new(feed, detail);

    s.handle_key(key(KeyCode::Char('a')));
    s.handle_key(key(KeyCode::Char('b')));
    s.handle_key(key(KeyCode::Left));
    s.handle_key(key(KeyCode::Delete));

    assert_eq!(s.input(), "a");
}

#[test]
fn it_does_not_pop_on_left_right() {
    let (feed, _) = search_feed();
    let detail: blockexplorer_tui::adapters::ui::DetailFactory =
        Box::new(|_: ResolvedEntity| unreachable!("not used"));
    let mut s = SearchScreen::new(feed, detail);

    assert_eq!(s.handle_key(key(KeyCode::Left)), Command::None);
    assert_eq!(s.handle_key(key(KeyCode::Right)), Command::None);
}
