//! Hierarchical focus layers for detail screens (main tabs, optional
//! sub-tabs, body). Border and tab-highlight styles use semantic
//! [`Palette`] tokens — see `plan/17-navigable-values.md`.

use ratatui::style::{Modifier, Style};

use super::theme::Palette;

/// Which region of a detail screen owns arrow keys before per-widget
/// dispatch runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetailFocusLayer {
    /// Arrow keys move field cursors, lists, and editors.
    #[default]
    Content,
    /// Left / right change main tabs; down enters sub-tabs or body.
    MainTabs,
    /// Left / right change sub-tabs when the main tab exposes a strip.
    Subtabs,
}

/// Which horizontal tab strip is being painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailTabStrip {
    Main,
    Sub,
}

/// Border style for a `Tabs` block from focus + strip role.
#[must_use]
pub fn tab_strip_border_style(
    focus: DetailFocusLayer,
    strip: DetailTabStrip,
    sub_strip_visible: bool,
    palette: &Palette,
) -> Style {
    let focused_strip = matches!(
        (focus, strip),
        (DetailFocusLayer::MainTabs, DetailTabStrip::Main)
            | (DetailFocusLayer::Subtabs, DetailTabStrip::Sub)
    );
    if focused_strip {
        Style::default()
            .fg(palette.accent)
            .add_modifier(Modifier::BOLD)
    } else if is_ancestor_strip(focus, strip, sub_strip_visible) {
        Style::default()
            .fg(palette.muted)
            .add_modifier(Modifier::DIM)
            .add_modifier(Modifier::BOLD)
    } else if matches!(
        (focus, strip),
        (DetailFocusLayer::MainTabs, DetailTabStrip::Sub)
    ) && sub_strip_visible
    {
        Style::default().fg(palette.muted)
    } else {
        Style::default().fg(palette.foreground)
    }
}

const fn is_ancestor_strip(
    focus: DetailFocusLayer,
    strip: DetailTabStrip,
    sub_strip_visible: bool,
) -> bool {
    match focus {
        DetailFocusLayer::MainTabs => false,
        DetailFocusLayer::Subtabs => matches!(strip, DetailTabStrip::Main),
        DetailFocusLayer::Content => {
            matches!(strip, DetailTabStrip::Main)
                || (matches!(strip, DetailTabStrip::Sub) && sub_strip_visible)
        }
    }
}

/// Highlight style for the selected tab inside [`ratatui::widgets::Tabs`].
#[must_use]
pub fn tab_strip_highlight_style(
    focus: DetailFocusLayer,
    strip: DetailTabStrip,
    sub_strip_visible: bool,
    palette: &Palette,
) -> Style {
    let focused_strip = matches!(
        (focus, strip),
        (DetailFocusLayer::MainTabs, DetailTabStrip::Main)
            | (DetailFocusLayer::Subtabs, DetailTabStrip::Sub)
    );
    if focused_strip {
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(palette.background)
            .bg(palette.accent)
    } else if is_ancestor_strip(focus, strip, sub_strip_visible) {
        Style::default()
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::DIM)
            .fg(palette.accent)
            .bg(palette.background)
    } else if matches!(
        (focus, strip),
        (DetailFocusLayer::MainTabs, DetailTabStrip::Sub)
    ) && sub_strip_visible
    {
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(palette.muted)
            .bg(palette.background)
    } else {
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(palette.foreground)
            .bg(palette.muted)
    }
}

/// Border style for the primary body pane (overview, list, chart, …).
#[must_use]
pub fn detail_body_border_style(focus: DetailFocusLayer, palette: &Palette) -> Style {
    if focus == DetailFocusLayer::Content {
        Style::default()
            .fg(palette.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        // Unfocused panes match inactive tab chrome (dim grey), not
        // `foreground` which reads as a strong white on dark palettes.
        Style::default().fg(palette.muted)
    }
}
