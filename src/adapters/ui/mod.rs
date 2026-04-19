//! TUI adapter: `ratatui` screens, widgets, input router.
//!
//! This module must not import other `adapters::*` modules. It depends on
//! the application through ports passed in by `infra`.
//!
//! See `.cursor/rules/tui.mdc` for the detailed contract.

pub mod address_detail;
pub mod block_detail;
pub mod contract_detail;
pub mod detail_placeholder;
pub mod format;
pub mod gas_tracker;
pub mod global_keymap;
pub mod highlight;
pub mod home;
pub mod keybind_modal;
pub mod mempool;
pub mod modal;
pub mod screen;
pub mod scroll;
pub mod search;
pub mod settings;
pub mod theme;
pub mod token_detail;
pub mod tx_detail;

pub use address_detail::{
    AddressDetailScreen, AddressFeed, AddressFeedSender, AddressTab, OpenContractFactory,
    OpenTokenFactory, address_feed,
};
pub use block_detail::{
    BlockDetailScreen, BlockFeed, BlockFeedSender, BlockTab, OpenTxFactory, block_feed,
};
pub use contract_detail::{
    ContractDetailScreen, ContractFeed, ContractFeedSender, ContractTab, contract_feed,
};
pub use detail_placeholder::DetailPlaceholderScreen;
pub use gas_tracker::{
    GasFeed, GasFeedSender, GasRefreshHandle, GasRefreshListener, GasTrackerScreen, gas_feed,
    gas_refresh_channel,
};
pub use global_keymap::{GlobalKeyMap, ModalFactory};
pub use home::{HomeFeed, HomeFeedSender, HomeScreen, home_feed};
pub use keybind_modal::KeyBindConflictModal;
pub use mempool::{MempoolScreen, OpenPendingTxFactory};
pub use modal::HelpModal;
pub use screen::{Command, Screen, ScreenStack, Transition};
pub use search::{
    DetailFactory, SearchFeed, SearchFeedSender, SearchFeedUpdate, SearchScreen, search_feed,
};
pub use settings::{AppConfigSnapshot, ProviderHealthSnapshot, SettingsScreen};
pub use theme::{Palette, PalettePreset};
pub use token_detail::{
    OpenContractFactory as TokenOpenContractFactory, OpenTxFactory as TokenOpenTxFactory,
    TokenDetailScreen, TokenFeed, TokenFeedSender, TokenTab, token_feed,
};
pub use tx_detail::{TxDetailScreen, TxFeed, TxFeedSender, TxTab, tx_feed};
