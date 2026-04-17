//! Cucumber `World` shared across every scenario.
//!
//! Holds the stubbed ports, the in-memory screen stack, and the last
//! rendered frame captured through `ratatui::backend::TestBackend`.
//!
//! During the scaffolding phase most fields are placeholders; they are
//! filled in as ports and use cases are introduced.

use cucumber::World;

#[derive(Debug, Default, World)]
pub struct AppWorld {
    /// Slug of the currently active chain. Defaults to `None` until a
    /// scenario sets it via the `Given the active chain is "..."` step.
    pub active_chain: Option<String>,

    /// Identifier of the current top-of-stack screen. Set by step
    /// implementations as the user navigates.
    pub current_screen: Option<String>,

    /// Latest arbitrary text rendered on screen. Used by assertions that
    /// check for a substring on the current frame.
    pub rendered: String,
}
