//! TUI adapter: `ratatui` screens, widgets, input router.
//!
//! This module must not import other `adapters::*` modules. It depends on
//! the application through ports passed in by `infra`.
//!
//! See `.cursor/rules/tui.mdc` for the detailed contract.

pub mod detail_placeholder;
pub mod home;
pub mod screen;
pub mod search;

pub use detail_placeholder::DetailPlaceholderScreen;
pub use home::{HomeFeed, HomeFeedSender, HomeScreen, home_feed};
pub use screen::{Command, Screen, ScreenStack};
pub use search::{
    DetailFactory, SearchFeed, SearchFeedSender, SearchFeedUpdate, SearchScreen,
    search_feed,
};
