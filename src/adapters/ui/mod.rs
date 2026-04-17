//! TUI adapter: `ratatui` screens, widgets, input router.
//!
//! This module must not import other `adapters::*` modules. It depends on
//! the application through ports passed in by `infra`.
//!
//! See `.cursor/rules/tui.mdc` for the detailed contract.

pub mod home;
pub mod screen;

pub use home::HomeScreen;
pub use screen::{Command, Screen, ScreenStack};
