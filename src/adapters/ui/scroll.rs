//! Shared bounded-scroll helper for every scrollable screen.
//!
//! See `plan/4-tx-detail.md` section 13.3.
//!
//! The widget-level `Paragraph::scroll((offset, 0))` accepts any
//! `u16` without clamping, which means screens can easily scroll
//! into "blank territory": past the end of a long content, or below
//! zero (where `saturating_sub` silently no-ops but still marks the
//! frame dirty). [`ScrollState`] centralises the three pieces of
//! state every screen needs (current offset, content height,
//! viewport height) and exposes an explicit `scroll_by` that clamps
//! the offset to `0..=max_offset()`.
//!
//! Screens call [`ScrollState::set_dimensions`] on every render with
//! the actual `content_height` (number of wrapped lines in their
//! body) and `viewport_height` (usable rows inside the bordered
//! content `Rect`). Key handlers route scroll input through
//! [`ScrollState::scroll_by`] / `page_up` / `page_down` / `home` /
//! `end`.

/// Bounded vertical scroll state for a scrollable widget.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScrollState {
    offset: u16,
    content_height: u16,
    viewport_height: u16,
}

impl ScrollState {
    /// Create a scroll state that is already clamped to zero.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            offset: 0,
            content_height: 0,
            viewport_height: 0,
        }
    }

    /// Update the content / viewport dimensions. Called on every
    /// render after measuring the body text and the bordered area.
    /// Re-clamps `offset` so resizing the terminal never leaves the
    /// cursor in a stale position.
    pub fn set_dimensions(&mut self, content_height: u16, viewport_height: u16) {
        self.content_height = content_height;
        self.viewport_height = viewport_height;
        let max = self.max_offset();
        if self.offset > max {
            self.offset = max;
        }
    }

    /// Current offset. Suitable for `Paragraph::scroll((offset, 0))`.
    #[must_use]
    pub const fn offset(&self) -> u16 {
        self.offset
    }

    /// Largest offset that still shows content on the last row of
    /// the viewport. When the content fits the viewport, the max
    /// offset is zero, which makes `is_scrollable()` return `false`.
    #[must_use]
    pub const fn max_offset(&self) -> u16 {
        self.content_height.saturating_sub(self.viewport_height)
    }

    /// True when the content overflows the viewport. Screens should
    /// skip their scroll-indicator chrome when this is `false`.
    #[must_use]
    pub const fn is_scrollable(&self) -> bool {
        self.content_height > self.viewport_height
    }

    /// Apply a signed delta to the offset, clamped to the valid
    /// range. Returns `true` when the offset actually changed, so
    /// the dispatcher can decide whether the frame needs a redraw.
    pub fn scroll_by(&mut self, delta: i32) -> bool {
        if !self.is_scrollable() && delta != 0 {
            return false;
        }
        let max = self.max_offset();
        let target = (i32::from(self.offset)).saturating_add(delta);
        let clamped = target.clamp(0, i32::from(max));
        let new_offset = clamped as u16;
        if new_offset == self.offset {
            false
        } else {
            self.offset = new_offset;
            true
        }
    }

    /// Scroll up by ten rows (or to the top, whichever comes first).
    pub fn page_up(&mut self) -> bool {
        self.scroll_by(-10)
    }

    /// Scroll down by ten rows (or to the bottom, whichever comes
    /// first).
    pub fn page_down(&mut self) -> bool {
        self.scroll_by(10)
    }

    /// Jump to the first row.
    pub fn home(&mut self) -> bool {
        let changed = self.offset != 0;
        self.offset = 0;
        changed
    }

    /// Jump to the last-visible row (the highest valid offset).
    pub fn end(&mut self) -> bool {
        let max = self.max_offset();
        let changed = self.offset != max;
        self.offset = max;
        changed
    }

    /// Adjust the offset so `row` is visible inside the viewport (used
    /// for line-wise selection that drives scroll).
    pub fn scroll_row_into_view(&mut self, row: u16) {
        if self.viewport_height == 0 {
            return;
        }
        if self.content_height == 0 {
            self.offset = 0;
            return;
        }
        let max = self.max_offset();
        let v = self.viewport_height;
        let mut off = self.offset;
        if row < off {
            off = row;
        } else if row >= off.saturating_add(v) {
            off = row + 1 - v;
        }
        self.offset = off.min(max);
    }

    /// Reset to the top without touching the cached dimensions.
    /// Called when the parent screen switches tabs or loads new
    /// content.
    pub fn reset(&mut self) {
        self.offset = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_state_is_not_scrollable() {
        let s = ScrollState::new();
        assert_eq!(s.offset(), 0);
        assert!(!s.is_scrollable());
        assert_eq!(s.max_offset(), 0);
    }

    #[test]
    fn content_shorter_than_viewport_is_not_scrollable() {
        let mut s = ScrollState::new();
        s.set_dimensions(5, 20);
        assert!(!s.is_scrollable());
        assert_eq!(s.max_offset(), 0);
        assert!(!s.scroll_by(3));
        assert_eq!(s.offset(), 0);
        assert!(!s.page_down());
    }

    #[test]
    fn content_exactly_equal_to_viewport_is_not_scrollable() {
        let mut s = ScrollState::new();
        s.set_dimensions(10, 10);
        assert!(!s.is_scrollable());
        assert_eq!(s.max_offset(), 0);
        assert!(!s.scroll_by(1));
    }

    #[test]
    fn scroll_by_clamps_to_max_offset() {
        let mut s = ScrollState::new();
        s.set_dimensions(100, 20);
        assert_eq!(s.max_offset(), 80);
        assert!(s.scroll_by(50));
        assert_eq!(s.offset(), 50);
        assert!(s.scroll_by(100));
        assert_eq!(s.offset(), 80);
        assert!(!s.scroll_by(5));
        assert_eq!(s.offset(), 80);
    }

    #[test]
    fn scroll_by_clamps_to_zero_on_negative_overflow() {
        let mut s = ScrollState::new();
        s.set_dimensions(100, 20);
        s.scroll_by(30);
        assert!(s.scroll_by(-100));
        assert_eq!(s.offset(), 0);
        assert!(!s.scroll_by(-1));
    }

    #[test]
    fn shrinking_the_viewport_reclamps_offset() {
        let mut s = ScrollState::new();
        s.set_dimensions(100, 20);
        s.scroll_by(80);
        assert_eq!(s.offset(), 80);
        s.set_dimensions(100, 90);
        assert_eq!(s.offset(), 10);
    }

    #[test]
    fn home_and_end_honour_the_clamp() {
        let mut s = ScrollState::new();
        s.set_dimensions(50, 10);
        assert!(s.end());
        assert_eq!(s.offset(), 40);
        assert!(!s.end());
        assert!(s.home());
        assert_eq!(s.offset(), 0);
        assert!(!s.home());
    }

    #[test]
    fn page_up_and_page_down_step_by_ten() {
        let mut s = ScrollState::new();
        s.set_dimensions(100, 20);
        assert!(s.page_down());
        assert_eq!(s.offset(), 10);
        assert!(s.page_down());
        assert_eq!(s.offset(), 20);
        assert!(s.page_up());
        assert_eq!(s.offset(), 10);
    }

    #[test]
    fn scroll_row_into_view_keeps_row_visible() {
        let mut s = ScrollState::new();
        s.set_dimensions(50, 10);
        s.scroll_row_into_view(25);
        assert_eq!(s.offset(), 16);
        s.scroll_row_into_view(5);
        assert_eq!(s.offset(), 5);
        s.scroll_row_into_view(12);
        assert_eq!(s.offset(), 5);
    }

    #[test]
    fn reset_returns_to_top_but_keeps_dimensions() {
        let mut s = ScrollState::new();
        s.set_dimensions(100, 20);
        s.scroll_by(50);
        s.reset();
        assert_eq!(s.offset(), 0);
        assert!(s.is_scrollable());
    }
}
