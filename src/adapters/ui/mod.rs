//! TUI adapter: `ratatui` screens, widgets, input router.
//!
//! This module must not import other `adapters::*` modules. It depends on
//! the application through ports passed in by `infra`.
//!
//! See `.cursor/rules/tui.mdc` for the detailed contract.

pub mod address_detail;
pub mod block_detail;
pub mod breadcrumb;
pub mod detail_placeholder;
pub mod field_cursor;
pub mod format;
pub mod global_keymap;
pub mod highlight;
pub mod home;
pub mod keybind_modal;
pub mod modal;
pub mod screen;
pub mod scroll;
pub mod search;
pub mod settings;
pub mod theme;
pub mod tx_detail;

pub use address_detail::{
    AddressDetailScreen, AddressFeed, AddressFeedSender, AddressTab, ContractSubTab,
    OpenTokenFactory, OpenTxFactory, TokenSubTab, address_feed,
};
pub use block_detail::{
    BlockDetailScreen, BlockFeed, BlockFeedSender, BlockTab, OpenTxFactory as BlockOpenTxFactory,
    block_feed,
};
pub use breadcrumb::{BREADCRUMB_SEPARATOR, breadcrumb_segments, render_breadcrumb};
pub use detail_placeholder::DetailPlaceholderScreen;
pub use field_cursor::{CursorDir, CursorServices, FieldCursor, FieldEntry, NavigationFactory};
pub use global_keymap::{GlobalKeyMap, ModalFactory};
pub use home::{HomeFeed, HomeFeedSender, HomeScreen, home_feed};
pub use keybind_modal::KeyBindConflictModal;
pub use modal::HelpModal;
pub use screen::{Command, Screen, ScreenStack, Transition};
pub use search::{
    DetailFactory, SearchFeed, SearchFeedSender, SearchFeedUpdate, SearchScreen, search_feed,
};
pub use settings::{AppConfigSnapshot, ProviderHealthSnapshot, SettingsScreen};
pub use theme::{Palette, PalettePreset};
pub use tx_detail::{TxDetailScreen, TxFeed, TxFeedSender, TxTab, tx_feed};
