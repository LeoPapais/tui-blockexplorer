//! State-machine tests for the universal field cursor.
//!
//! The widget is rendered-agnostic; these tests drive it via
//! [`FieldCursor::move_in`] directly and assert on
//! [`FieldCursor::active`] / [`FieldCursor::current`]. See
//! `plan/17-navigable-values.md` §4 and §8.1.

use blockexplorer_tui::{
    adapters::ui::{CursorDir, FieldCursor, FieldEntry},
    domain::{BlockNumber, NavigableValue},
};
use pretty_assertions::assert_eq;

fn sample_fields() -> Vec<FieldEntry> {
    vec![
        FieldEntry::new(
            "latest_block",
            NavigableValue::BlockNumber(BlockNumber::new(21_000_000)),
        ),
        FieldEntry::new("chain", NavigableValue::Plain("ethereum".to_string())),
        FieldEntry::new("gas", NavigableValue::Plain("12 gwei".to_string())),
    ]
}

#[test]
fn cursor_starts_inactive() {
    let cursor = FieldCursor::new();
    assert!(!cursor.is_active());
    assert_eq!(cursor.active(), None);
    assert!(cursor.current(&sample_fields()).is_none());
}

#[test]
fn cursor_activates_on_first_arrow_key() {
    let fields = sample_fields();
    let mut cursor = FieldCursor::new();
    cursor.move_in(fields.len(), CursorDir::Right);
    assert!(cursor.is_active());
    assert_eq!(cursor.active(), Some(0));
    let current = cursor.current(&fields).expect("cursor should have a field");
    assert_eq!(current.label, "latest_block");
}

#[test]
fn right_arrow_moves_to_next_field_in_reading_order() {
    let fields = sample_fields();
    let mut cursor = FieldCursor::new();
    cursor.move_in(fields.len(), CursorDir::Right);
    cursor.move_in(fields.len(), CursorDir::Right);
    assert_eq!(cursor.active(), Some(1));
    let current = cursor
        .current(&fields)
        .expect("cursor should point at chain");
    assert_eq!(current.label, "chain");
}

#[test]
fn down_arrow_advances_just_like_right() {
    let fields = sample_fields();
    let mut cursor = FieldCursor::new();
    cursor.move_in(fields.len(), CursorDir::Down);
    cursor.move_in(fields.len(), CursorDir::Down);
    cursor.move_in(fields.len(), CursorDir::Down);
    assert_eq!(cursor.active(), Some(2));
}

#[test]
fn wrap_around_at_edges_is_a_noop() {
    let fields = sample_fields();
    let mut cursor = FieldCursor::new();
    cursor.move_in(fields.len(), CursorDir::Right);
    for _ in 0..10 {
        cursor.move_in(fields.len(), CursorDir::Right);
    }
    assert_eq!(
        cursor.active(),
        Some(fields.len() - 1),
        "right past the end should saturate"
    );
    for _ in 0..10 {
        cursor.move_in(fields.len(), CursorDir::Left);
    }
    assert_eq!(
        cursor.active(),
        Some(0),
        "left past the start should saturate"
    );
}

#[test]
fn empty_field_list_deactivates_the_cursor() {
    let mut cursor = FieldCursor::new();
    cursor.move_in(3, CursorDir::Right);
    cursor.move_in(0, CursorDir::Right);
    assert_eq!(cursor.active(), None);
    assert!(!cursor.is_active());
}

#[test]
fn esc_deactivates_cursor_without_popping_screen() {
    let mut cursor = FieldCursor::new();
    cursor.move_in(3, CursorDir::Right);
    assert!(cursor.is_active());
    cursor.deactivate();
    assert!(!cursor.is_active());
    assert_eq!(cursor.active(), None);
}
