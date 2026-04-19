//! Pure helper that renders a breadcrumb string from a [`ScreenStack`].
//!
//! `plan/0-general-architecture.md` §3 describes the shell chrome as
//! a header, a breadcrumb row, the content area and a status bar.
//! Today only the content area is rendered; this helper is the first
//! ingredient for the breadcrumb row.
//!
//! The helper is pure (no I/O, no `self`) so it can be unit-tested
//! without instantiating a live runtime. Callers should feed it the
//! real [`ScreenStack`] and display the returned string verbatim.
//!
//! Contract:
//!
//! - The breadcrumb reads bottom-up: `Home > Block 123 > Tx 0xabc…`
//!   (root first, current screen last). This mirrors a filesystem
//!   path, which matches users' mental model of "where am I right
//!   now" more naturally than the reverse.
//! - Modals are deliberately ignored; the breadcrumb describes the
//!   back-stack the user would see once the modal closes.
//! - Empty stacks render as the empty string, so the shell can
//!   conditionally hide the breadcrumb row when there is nothing to
//!   show.
//!
//! See `plan/15-backlog.md` §8.16 "Application shell" (breadcrumb
//! trail from the screen stack).

use super::ScreenStack;

/// Character inserted between crumbs. Matches the ASCII `>` used in
/// the mockup inside `plan/0-general-architecture.md` §3; callers
/// that need a nicer glyph can build their own render on top of
/// [`breadcrumb_segments`].
pub const BREADCRUMB_SEPARATOR: &str = " > ";

/// Return the breadcrumb titles bottom-up (root first).
///
/// Pure helper exposed alongside [`render_breadcrumb`] for callers
/// that want to colour individual crumbs differently in the UI
/// (accent for the current screen, dim for everything else).
pub fn breadcrumb_segments(stack: &ScreenStack) -> Vec<String> {
    stack.titles().map(str::to_owned).collect()
}

/// Render the breadcrumb trail for the current [`ScreenStack`].
///
/// The output is ` > `-joined, bottom-up, with no leading or trailing
/// separators. An empty stack returns an empty string. The helper
/// never panics.
#[must_use]
pub fn render_breadcrumb(stack: &ScreenStack) -> String {
    breadcrumb_segments(stack).join(BREADCRUMB_SEPARATOR)
}
