//! Address Detail screen (unified — see plan/16).
//!
//! Single screen that replaces the three previous detail screens
//! (`AddressDetailScreen`, `ContractDetailScreen`,
//! `TokenDetailScreen`). Main tabs adapt to the loaded address:
//!
//! - **EOA**: `Overview`, `Transactions`, `Transfers`, `Tokens`.
//! - **Plain contract**: same three + `Contract` (sub-tabs Source,
//!   ABI, Read, Events, Storage — the `Contract/Overview` sub-tab
//!   keeps the contract dossier with proxy + compiler metadata).
//! - **ERC-20**: same four + `Token` (sub-tabs Overview, Transfers,
//!   Chart).
//!
//! Sub-tabs draw as a second tabs row below the main tabs row when
//! `active_tab` is `Contract` or `Token`. `Tab`/`Shift+Tab` cycle
//! the main tabs when focus is on the body or the main tab strip;
//! when focus is on the sub-tab strip they cycle sub-tabs only.
//! `]`/`[` still cycle sub-tabs from any layer.
//!
//! The screen owns every channel that fed the three prior screens.
//! See `plan/16-unified-address-detail.md` §4–§6 for the data
//! plumbing and the lazy-dispatch rules followed by
//! `src/infra/address_feed::spawn`.

use std::{any::Any, collections::HashMap};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, Block, Borders, Chart, Dataset, GraphType, List, ListItem, ListState, Paragraph,
        Tabs, Wrap,
    },
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::{
        detail_focus::{
            DetailFocusLayer, DetailTabStrip, detail_body_border_style, tab_strip_border_style,
            tab_strip_highlight_style,
        },
        field_cursor::{CursorDir, CursorServices, FieldCursor, FieldEntry},
        highlight::highlight_solidity,
        screen::{Command, Screen},
        scroll::ScrollState,
        theme::{Palette, PalettePreset},
    },
    domain::{
        AbiFunction, AbiParamType, AbiValue, AccountTx, AccountTxPage, Address, AddressKind,
        AddressOverview, BlockNumber, Chain, ContractOverview, ContractSource, DecodedValue,
        DomainError, EventsPage, NavigableValue, NftKind, PriceLookup, PricePoint, PriceSeries,
        PriceWindow, SourceFile, TokenHolding, TokenOverview, TokenPrice, TransferAsset,
        TransferEvent, TransferPage, TxHash, parse_abi_functions,
    },
};

// ---------------------------------------------------------------------------
// On-demand requests (formerly lived in src/adapters/ui/contract_detail.rs)
// ---------------------------------------------------------------------------

/// Which ABI artifact was used to build calldata for a Read request.
///
/// The background feed always passes the **user-facing** address (the
/// proxy/listener contract) as `to` for `eth_call`. When the active tab
/// is **Contract (impl)**, calldata is derived from the implementation
/// contract ABI while `to` remains the proxy — matching `delegatecall`
/// semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadCalldataSource {
    /// ABI loaded for the proxy contract address (main Contract tab).
    ProxyArtifact,
    /// ABI loaded for the detected implementation (Contract impl tab).
    ImplementationArtifact,
}

/// Single-request envelope for the Read sub-tab.
#[derive(Debug, Clone)]
pub struct ReadRequest {
    pub function: AbiFunction,
    pub args: Vec<AbiValue>,
    pub calldata_source: ReadCalldataSource,
}

/// Async result for Read: routes to the correct tab state by calldata source.
#[derive(Debug)]
pub struct ReadDelivery {
    pub calldata_source: ReadCalldataSource,
    pub signature: String,
    pub result: ReadResult,
}

/// Result of a single read invocation.
pub type ReadResult = Result<Vec<DecodedValue>, DomainError>;

/// Request shape for the Events sub-tab.
#[derive(Debug, Clone, Copy)]
pub struct EventsRequest {
    pub head_hint: Option<BlockNumber>,
    pub offset: u32,
}

/// Result of a single Events sub-tab refresh.
pub type EventsResult = Result<EventsPage, DomainError>;

/// Request shape for the Storage sub-tab.
#[derive(Debug, Clone)]
pub struct StorageRequest {
    pub slot: [u8; 32],
}

/// Result of a single Storage sub-tab read.
pub type StorageResult = Result<[u8; 32], DomainError>;

// ---------------------------------------------------------------------------
// Feed / sender
// ---------------------------------------------------------------------------

/// Channel half owned by the screen. The UI drains every `*_rx`
/// inside `tick`, and emits requests on every `*_tx` in response to
/// keypresses. See plan/16 §4.
pub struct AddressFeed {
    pub input_tx: UnboundedSender<Address>,
    // Always-on channels.
    pub updates_rx: UnboundedReceiver<AddressOverview>,
    /// Normal transactions (`txlist`) for the **Transactions** tab.
    pub account_transactions_rx: UnboundedReceiver<AccountTxPage>,
    /// Asset / ERC-20 transfers for the **Transfers** tab.
    pub transfers_rx: UnboundedReceiver<TransferPage>,
    pub portfolio_rx: UnboundedReceiver<Vec<TokenHolding>>,
    // ERC-20 gated.
    pub token_overview_rx: UnboundedReceiver<Option<TokenOverview>>,
    pub token_price_rx: UnboundedReceiver<PriceLookup>,
    pub token_series_rx: UnboundedReceiver<PriceSeries>,
    pub token_window_req_tx: UnboundedSender<PriceWindow>,
    pub token_transfers_rx: UnboundedReceiver<TransferPage>,
    // Contract gated.
    pub contract_overview_rx: UnboundedReceiver<ContractOverview>,
    pub source_rx: UnboundedReceiver<ContractSource>,
    /// Implementation contract overview (only for proxy addresses).
    pub contract_impl_overview_rx: UnboundedReceiver<ContractOverview>,
    /// Source + ABI for [`ContractOverview::proxy`]'s implementation.
    pub source_impl_rx: UnboundedReceiver<ContractSource>,
    pub read_tx: UnboundedSender<ReadRequest>,
    pub read_rx: UnboundedReceiver<ReadDelivery>,
    pub events_tx: UnboundedSender<EventsRequest>,
    pub events_rx: UnboundedReceiver<EventsResult>,
    pub storage_tx: UnboundedSender<StorageRequest>,
    pub storage_rx: UnboundedReceiver<StorageResult>,
}

/// Channel half owned by the background task.
pub struct AddressFeedSender {
    pub updates_tx: UnboundedSender<AddressOverview>,
    pub account_transactions_tx: UnboundedSender<AccountTxPage>,
    pub transfers_tx: UnboundedSender<TransferPage>,
    pub portfolio_tx: UnboundedSender<Vec<TokenHolding>>,
    pub token_overview_tx: UnboundedSender<Option<TokenOverview>>,
    pub token_price_tx: UnboundedSender<PriceLookup>,
    pub token_series_tx: UnboundedSender<PriceSeries>,
    pub token_window_req_rx: UnboundedReceiver<PriceWindow>,
    pub token_transfers_tx: UnboundedSender<TransferPage>,
    pub contract_overview_tx: UnboundedSender<ContractOverview>,
    pub source_tx: UnboundedSender<ContractSource>,
    pub contract_impl_overview_tx: UnboundedSender<ContractOverview>,
    pub source_impl_tx: UnboundedSender<ContractSource>,
    pub read_rx: UnboundedReceiver<ReadRequest>,
    pub read_tx: UnboundedSender<ReadDelivery>,
    pub events_rx: UnboundedReceiver<EventsRequest>,
    pub events_tx: UnboundedSender<EventsResult>,
    pub storage_rx: UnboundedReceiver<StorageRequest>,
    pub storage_tx: UnboundedSender<StorageResult>,
    pub input_rx: UnboundedReceiver<Address>,
}

#[must_use]
pub fn address_feed() -> (AddressFeed, AddressFeedSender) {
    let (input_tx, input_rx) = unbounded_channel();
    let (updates_tx, updates_rx) = unbounded_channel();
    let (account_transactions_tx, account_transactions_rx) = unbounded_channel();
    let (transfers_tx, transfers_rx) = unbounded_channel();
    let (portfolio_tx, portfolio_rx) = unbounded_channel();
    let (token_overview_tx, token_overview_rx) = unbounded_channel();
    let (token_price_tx, token_price_rx) = unbounded_channel();
    let (token_series_tx, token_series_rx) = unbounded_channel();
    let (token_window_req_tx, token_window_req_rx) = unbounded_channel();
    let (token_transfers_tx, token_transfers_rx) = unbounded_channel();
    let (contract_overview_tx, contract_overview_rx) = unbounded_channel();
    let (source_tx, source_rx) = unbounded_channel();
    let (contract_impl_overview_tx, contract_impl_overview_rx) = unbounded_channel();
    let (source_impl_tx, source_impl_rx) = unbounded_channel();
    let (read_req_tx, read_req_rx) = unbounded_channel();
    let (read_res_tx, read_res_rx) = unbounded_channel();
    let (events_req_tx, events_req_rx) = unbounded_channel();
    let (events_res_tx, events_res_rx) = unbounded_channel();
    let (storage_req_tx, storage_req_rx) = unbounded_channel();
    let (storage_res_tx, storage_res_rx) = unbounded_channel();
    (
        AddressFeed {
            input_tx,
            updates_rx,
            account_transactions_rx,
            transfers_rx,
            portfolio_rx,
            token_overview_rx,
            token_price_rx,
            token_series_rx,
            token_window_req_tx,
            token_transfers_rx,
            contract_overview_rx,
            source_rx,
            contract_impl_overview_rx,
            source_impl_rx,
            read_tx: read_req_tx,
            read_rx: read_res_rx,
            events_tx: events_req_tx,
            events_rx: events_res_rx,
            storage_tx: storage_req_tx,
            storage_rx: storage_res_rx,
        },
        AddressFeedSender {
            updates_tx,
            account_transactions_tx,
            transfers_tx,
            portfolio_tx,
            token_overview_tx,
            token_price_tx,
            token_series_tx,
            token_window_req_rx,
            token_transfers_tx,
            contract_overview_tx,
            source_tx,
            contract_impl_overview_tx,
            source_impl_tx,
            read_rx: read_req_rx,
            read_tx: read_res_tx,
            events_rx: events_req_rx,
            events_tx: events_res_tx,
            storage_rx: storage_req_rx,
            storage_tx: storage_res_tx,
            input_rx,
        },
    )
}

/// Factory used by list-based tabs to spawn a TxDetail screen when
/// the user presses Enter on a row.
pub type OpenTxFactory = Box<dyn Fn(TxHash) -> Box<dyn Screen> + Send + Sync>;

/// Factory used by the Tokens main tab (and by the Token/Transfers
/// sub-tab) to open another AddressDetail when the user presses
/// Enter on a holding. Kept under the old "token" name so existing
/// BDD helpers keep compiling; the returned screen is always an
/// `AddressDetailScreen`.
pub type OpenTokenFactory = Box<dyn Fn(Address) -> Box<dyn Screen> + Send + Sync>;

// ---------------------------------------------------------------------------
// Tabs / sub-tabs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressTab {
    Overview,
    /// Executed normal transactions (`txlist`).
    Transactions,
    /// Asset / token transfers (`alchemy_getAssetTransfers`).
    Transfers,
    Tokens,
    /// Visible only when the ERC-20 probe confirmed `IsToken`.
    Token,
    /// Visible only when the loaded overview reports `Contract`.
    Contract,
    /// Visible when [`ContractOverview::proxy`] is present: same
    /// sub-tabs as `Contract`, but source/ABI/overview default to the
    /// implementation address. Read uses implementation ABI with
    /// `eth_call` `to` = proxy (see [`ReadCalldataSource`]).
    ContractImpl,
}

impl AddressTab {
    fn label(self) -> &'static str {
        match self {
            AddressTab::Overview => "Overview",
            AddressTab::Transactions => "Transactions",
            AddressTab::Transfers => "Transfers",
            AddressTab::Tokens => "Tokens",
            AddressTab::Token => "Token",
            AddressTab::Contract => "Contract",
            AddressTab::ContractImpl => "Impl",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractSubTab {
    Overview,
    Source,
    Abi,
    Read,
    Events,
    Storage,
}

impl ContractSubTab {
    const ALL: [ContractSubTab; 6] = [
        ContractSubTab::Overview,
        ContractSubTab::Source,
        ContractSubTab::Abi,
        ContractSubTab::Read,
        ContractSubTab::Events,
        ContractSubTab::Storage,
    ];

    fn index(self) -> usize {
        match self {
            ContractSubTab::Overview => 0,
            ContractSubTab::Source => 1,
            ContractSubTab::Abi => 2,
            ContractSubTab::Read => 3,
            ContractSubTab::Events => 4,
            ContractSubTab::Storage => 5,
        }
    }

    fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    fn previous(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    fn label(self) -> &'static str {
        match self {
            ContractSubTab::Overview => "Overview",
            ContractSubTab::Source => "Source",
            ContractSubTab::Abi => "ABI",
            ContractSubTab::Read => "Read",
            ContractSubTab::Events => "Events",
            ContractSubTab::Storage => "Storage",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSubTab {
    Overview,
    Transfers,
    Chart,
}

impl TokenSubTab {
    pub const ALL: [TokenSubTab; 3] = [
        TokenSubTab::Overview,
        TokenSubTab::Transfers,
        TokenSubTab::Chart,
    ];

    fn index(self) -> usize {
        match self {
            TokenSubTab::Overview => 0,
            TokenSubTab::Transfers => 1,
            TokenSubTab::Chart => 2,
        }
    }

    fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    fn previous(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    fn label(self) -> &'static str {
        match self {
            TokenSubTab::Overview => "Overview",
            TokenSubTab::Transfers => "Transfers",
            TokenSubTab::Chart => "Chart",
        }
    }
}

/// Tri-state for the inline Token tab.
#[derive(Debug, Clone, PartialEq)]
enum TokenProbeState {
    Unknown,
    NotToken,
    IsToken(TokenOverview),
}

/// Focus within the Read sub-tab: function list, argument editor,
/// or the result pane (scrollable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadFocus {
    FunctionList,
    Args,
    Result,
}

/// Focus within the Source sub-tab: file list vs source viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceFocus {
    Files,
    Viewer,
}

/// Focus within the Storage sub-tab: slot input vs value body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StorageFocus {
    Slot,
    Value,
}

// ---------------------------------------------------------------------------
// Screen
// ---------------------------------------------------------------------------

pub struct AddressDetailScreen {
    #[allow(dead_code)]
    chain: Chain,
    address: Address,

    current: Option<AddressOverview>,
    /// First page of normal transactions (Etherscan `txlist`).
    account_tx_page: Option<AccountTxPage>,
    /// Asset transfers for the **Transfers** tab.
    transfers: Option<TransferPage>,
    holdings: Option<Vec<TokenHolding>>,

    // Token-gated state.
    token_probe: TokenProbeState,
    token_price: PriceLookup,
    token_series: HashMap<PriceWindow, PriceSeries>,
    token_transfers: Option<TransferPage>,
    active_token_window: PriceWindow,
    live_samples: usize,

    // Contract-gated state (proxy / user-facing contract).
    contract_overview: Option<ContractOverview>,
    source: Option<ContractSource>,
    functions: Vec<AbiFunction>,
    function_list_state: ListState,
    file_list_state: ListState,
    /// Implementation dossier when `contract_overview` reports a proxy.
    contract_overview_impl: Option<ContractOverview>,
    source_impl: Option<ContractSource>,
    functions_impl: Vec<AbiFunction>,
    function_list_state_impl: ListState,
    file_list_state_impl: ListState,
    read_focus: ReadFocus,
    read_focus_impl: ReadFocus,
    /// Scroll offset for the Read tab result pane (independent of
    /// [`AddressDetailScreen::contract_scroll`]).
    read_result_scroll: std::cell::Cell<ScrollState>,
    read_result_scroll_impl: std::cell::Cell<ScrollState>,
    source_focus: SourceFocus,
    source_focus_impl: SourceFocus,
    storage_focus: StorageFocus,
    storage_value_scroll: std::cell::Cell<ScrollState>,
    /// Selected line in the Storage value pane (copy + highlight).
    storage_value_line: usize,
    arg_buffers: Vec<String>,
    arg_cursor: usize,
    arg_buffers_impl: Vec<String>,
    arg_cursor_impl: usize,
    last_result: Option<Result<Vec<DecodedValue>, String>>,
    last_result_for: Option<String>,
    last_result_impl: Option<Result<Vec<DecodedValue>, String>>,
    last_result_for_impl: Option<String>,
    events: Option<Result<EventsPage, String>>,
    events_offset: u32,
    events_head: Option<BlockNumber>,
    events_requested: bool,
    slot_buffer: String,
    storage_result: Option<Result<[u8; 32], String>>,
    storage_slot_requested: Option<[u8; 32]>,

    // UI state.
    feed: AddressFeed,
    active_tab: AddressTab,
    focus_layer: DetailFocusLayer,
    active_contract_sub: ContractSubTab,
    active_token_sub: TokenSubTab,
    scroll: u16,
    scroll_cap: std::cell::Cell<u16>,
    contract_scroll: std::cell::Cell<ScrollState>,
    impl_contract_scroll: std::cell::Cell<ScrollState>,

    tx_list_state: ListState,
    transfers_list_state: ListState,
    token_list_state: ListState,
    token_transfers_list_state: ListState,

    open_tx: Option<OpenTxFactory>,
    open_token: Option<OpenTokenFactory>,
    last_copied_value: Option<String>,
    /// Field cursor for the Overview tab. Inactive by default; the
    /// user activates it by pressing an arrow key. See
    /// `plan/17-navigable-values.md` §4.
    cursor: FieldCursor,
    cursor_services: Option<CursorServices>,
}

impl AddressDetailScreen {
    /// Loading constructor without any factories wired.
    #[must_use]
    pub fn loading(chain: Chain, address: Address, feed: AddressFeed) -> Self {
        Self::with_factories_and_tab(chain, address, feed, None, None, AddressTab::Overview)
    }

    /// Loading constructor that wires Enter on the Transactions /
    /// Token-Transfers lists to a TxDetail factory.
    #[must_use]
    pub fn with_open_tx(
        chain: Chain,
        address: Address,
        feed: AddressFeed,
        open_tx: Option<OpenTxFactory>,
    ) -> Self {
        Self::with_factories_and_tab(chain, address, feed, open_tx, None, AddressTab::Overview)
    }

    /// Fully-wired constructor (default main tab = Overview).
    #[must_use]
    pub fn with_factories(
        chain: Chain,
        address: Address,
        feed: AddressFeed,
        open_tx: Option<OpenTxFactory>,
        open_token: Option<OpenTokenFactory>,
    ) -> Self {
        Self::with_factories_and_tab(
            chain,
            address,
            feed,
            open_tx,
            open_token,
            AddressTab::Overview,
        )
    }

    /// Fully-wired constructor with an initial main tab. The tab is
    /// applied blindly even before the overview has loaded: if the
    /// tab is not in `visible_tabs()` yet, rendering falls back to
    /// the first visible tab until the feeds confirm the tab is
    /// available.
    #[must_use]
    pub fn with_factories_and_tab(
        chain: Chain,
        address: Address,
        feed: AddressFeed,
        open_tx: Option<OpenTxFactory>,
        open_token: Option<OpenTokenFactory>,
        initial_tab: AddressTab,
    ) -> Self {
        let _ = feed.input_tx.send(address);
        let mut tx_list_state = ListState::default();
        tx_list_state.select(Some(0));
        let mut token_list_state = ListState::default();
        token_list_state.select(Some(0));
        let mut transfers_list_state = ListState::default();
        transfers_list_state.select(Some(0));
        let mut token_transfers_list_state = ListState::default();
        token_transfers_list_state.select(Some(0));
        let mut function_list_state = ListState::default();
        function_list_state.select(Some(0));
        let mut file_list_state = ListState::default();
        file_list_state.select(Some(0));
        let mut function_list_state_impl = ListState::default();
        function_list_state_impl.select(Some(0));
        let mut file_list_state_impl = ListState::default();
        file_list_state_impl.select(Some(0));
        Self {
            chain,
            address,
            current: None,
            account_tx_page: None,
            transfers: None,
            holdings: None,
            token_probe: TokenProbeState::Unknown,
            token_price: PriceLookup::Pending,
            token_series: HashMap::new(),
            token_transfers: None,
            active_token_window: PriceWindow::D1,
            live_samples: 0,
            contract_overview: None,
            source: None,
            functions: Vec::new(),
            function_list_state,
            file_list_state,
            contract_overview_impl: None,
            source_impl: None,
            functions_impl: Vec::new(),
            function_list_state_impl,
            file_list_state_impl,
            read_focus: ReadFocus::FunctionList,
            read_focus_impl: ReadFocus::FunctionList,
            read_result_scroll: std::cell::Cell::new(ScrollState::new()),
            read_result_scroll_impl: std::cell::Cell::new(ScrollState::new()),
            source_focus: SourceFocus::Files,
            source_focus_impl: SourceFocus::Files,
            storage_focus: StorageFocus::Slot,
            storage_value_scroll: std::cell::Cell::new(ScrollState::new()),
            storage_value_line: 0,
            arg_buffers: Vec::new(),
            arg_cursor: 0,
            arg_buffers_impl: Vec::new(),
            arg_cursor_impl: 0,
            last_result: None,
            last_result_for: None,
            last_result_impl: None,
            last_result_for_impl: None,
            events: None,
            events_offset: 0,
            events_head: None,
            events_requested: false,
            slot_buffer: "0".to_string(),
            storage_result: None,
            storage_slot_requested: None,
            feed,
            active_tab: initial_tab,
            focus_layer: DetailFocusLayer::Content,
            active_contract_sub: ContractSubTab::Overview,
            active_token_sub: TokenSubTab::Overview,
            scroll: 0,
            scroll_cap: std::cell::Cell::new(0),
            contract_scroll: std::cell::Cell::new(ScrollState::new()),
            impl_contract_scroll: std::cell::Cell::new(ScrollState::new()),
            tx_list_state,
            transfers_list_state,
            token_list_state,
            token_transfers_list_state,
            open_tx,
            open_token,
            last_copied_value: None,
            cursor: FieldCursor::new(),
            cursor_services: None,
        }
    }

    /// Wire cursor-owned clipboard + navigation. See
    /// `plan/17-navigable-values.md` §4.
    #[must_use]
    pub fn with_cursor_services(mut self, services: CursorServices) -> Self {
        self.cursor_services = Some(services);
        self
    }

    /// Current cursor state. Exposed for tests.
    #[must_use]
    pub const fn cursor(&self) -> &FieldCursor {
        &self.cursor
    }

    /// Navigable values on the current tab. Populated for Overview
    /// (address, ENS, delegated_to) and Tokens (each holding's
    /// contract address). Other tabs return an empty list until the
    /// per-sub-tab follow-ups covered by `plan/15-backlog.md` §8.16
    /// are delivered.
    #[must_use]
    pub fn navigable_fields(&self) -> Vec<FieldEntry> {
        match self.active_tab_or_fallback() {
            AddressTab::Overview => {
                let Some(ov) = self.current.as_ref() else {
                    return Vec::new();
                };
                let mut fields = vec![FieldEntry::new(
                    "address",
                    NavigableValue::Address(ov.address),
                )];
                if let Some(ens) = ov.ens_name.as_ref() {
                    fields.push(FieldEntry::new("ens", NavigableValue::EnsName(ens.clone())));
                }
                if let Some(delegate) = ov.delegated_to {
                    fields.push(FieldEntry::new(
                        "delegated_to",
                        NavigableValue::Address(delegate),
                    ));
                }
                fields
            }
            AddressTab::Tokens => self
                .holdings
                .as_ref()
                .map(|items| {
                    items
                        .iter()
                        .map(|h| {
                            FieldEntry::new(
                                "token_address",
                                NavigableValue::TokenAddress(h.metadata.address),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            AddressTab::Contract
                if matches!(self.active_contract_sub, ContractSubTab::Overview) =>
            {
                let mut fields = vec![FieldEntry::new(
                    "address",
                    NavigableValue::Address(self.address),
                )];
                if let Some(ov) = self.contract_overview.as_ref()
                    && let Some(ref px) = ov.proxy
                {
                    fields.push(FieldEntry::new(
                        "implementation",
                        NavigableValue::Address(px.implementation),
                    ));
                }
                fields
            }
            AddressTab::ContractImpl
                if matches!(self.active_contract_sub, ContractSubTab::Overview) =>
            {
                let mut fields = Vec::new();
                if let Some(ov) = self.contract_overview_impl.as_ref() {
                    fields.push(FieldEntry::new(
                        "address",
                        NavigableValue::Address(ov.account.address),
                    ));
                }
                fields.push(FieldEntry::new(
                    "proxy",
                    NavigableValue::Address(self.address),
                ));
                fields
            }
            _ => Vec::new(),
        }
    }

    fn visible_tabs(&self) -> Vec<AddressTab> {
        let mut tabs = vec![
            AddressTab::Overview,
            AddressTab::Transactions,
            AddressTab::Transfers,
            AddressTab::Tokens,
        ];
        if matches!(self.token_probe, TokenProbeState::IsToken(_)) {
            tabs.push(AddressTab::Token);
        }
        if let Some(ov) = self.current.as_ref()
            && matches!(ov.kind, AddressKind::Contract)
        {
            tabs.push(AddressTab::Contract);
            if self.contract_overview.as_ref().and_then(|c| c.proxy).is_some() {
                tabs.push(AddressTab::ContractImpl);
            }
        }
        tabs
    }

    #[must_use]
    fn contract_main_is_impl(&self) -> bool {
        matches!(self.active_tab_or_fallback(), AddressTab::ContractImpl)
    }

    #[must_use]
    fn contract_strip_like_contract(&self) -> bool {
        matches!(
            self.active_tab_or_fallback(),
            AddressTab::Contract | AddressTab::ContractImpl
        )
    }

    #[must_use]
    fn read_focus_active(&self) -> ReadFocus {
        if self.contract_main_is_impl() {
            self.read_focus_impl
        } else {
            self.read_focus
        }
    }

    fn set_read_focus_active(&mut self, v: ReadFocus) {
        if self.contract_main_is_impl() {
            self.read_focus_impl = v;
        } else {
            self.read_focus = v;
        }
    }

    #[must_use]
    fn source_focus_active(&self) -> SourceFocus {
        if self.contract_main_is_impl() {
            self.source_focus_impl
        } else {
            self.source_focus
        }
    }

    fn set_source_focus_active(&mut self, v: SourceFocus) {
        if self.contract_main_is_impl() {
            self.source_focus_impl = v;
        } else {
            self.source_focus = v;
        }
    }

    fn active_tab_or_fallback(&self) -> AddressTab {
        let tabs = self.visible_tabs();
        if tabs.contains(&self.active_tab) {
            self.active_tab
        } else {
            tabs.first().copied().unwrap_or(AddressTab::Overview)
        }
    }

    fn next_tab(&self) -> AddressTab {
        let tabs = self.visible_tabs();
        let idx = tabs.iter().position(|&t| t == self.active_tab).unwrap_or(0);
        tabs[(idx + 1) % tabs.len()]
    }

    fn prev_tab(&self) -> AddressTab {
        let tabs = self.visible_tabs();
        let idx = tabs.iter().position(|&t| t == self.active_tab).unwrap_or(0);
        tabs[(idx + tabs.len() - 1) % tabs.len()]
    }

    #[must_use]
    pub fn tabs(&self) -> Vec<AddressTab> {
        self.visible_tabs()
    }

    #[must_use]
    pub fn current(&self) -> Option<&AddressOverview> {
        self.current.as_ref()
    }

    /// Asset / token transfer feed (main **Transfers** tab).
    #[must_use]
    pub fn transfers(&self) -> Option<&TransferPage> {
        self.transfers.as_ref()
    }

    /// Executed normal transactions (**Transactions** tab).
    #[must_use]
    pub fn account_transactions(&self) -> Option<&AccountTxPage> {
        self.account_tx_page.as_ref()
    }

    #[must_use]
    pub fn holdings(&self) -> Option<&Vec<TokenHolding>> {
        self.holdings.as_ref()
    }

    #[must_use]
    pub fn active_tab(&self) -> AddressTab {
        self.active_tab
    }

    /// Keyboard focus region for hierarchical tab navigation.
    #[must_use]
    pub const fn focus_layer(&self) -> DetailFocusLayer {
        self.focus_layer
    }

    #[must_use]
    pub fn active_contract_sub(&self) -> ContractSubTab {
        self.active_contract_sub
    }

    #[must_use]
    pub fn active_token_sub(&self) -> TokenSubTab {
        self.active_token_sub
    }

    #[must_use]
    pub fn token_overview(&self) -> Option<&TokenOverview> {
        match &self.token_probe {
            TokenProbeState::IsToken(ov) => Some(ov),
            _ => None,
        }
    }

    #[must_use]
    pub fn token_price(&self) -> Option<&TokenPrice> {
        self.token_price.as_available()
    }

    #[must_use]
    pub fn token_price_lookup(&self) -> &PriceLookup {
        &self.token_price
    }

    #[must_use]
    pub fn token_series(&self) -> Option<&PriceSeries> {
        self.token_series.get(&self.active_token_window)
    }

    #[must_use]
    pub fn active_window(&self) -> PriceWindow {
        self.active_token_window
    }

    #[must_use]
    pub fn token_transfers(&self) -> Option<&TransferPage> {
        self.token_transfers.as_ref()
    }

    #[must_use]
    pub fn live_samples_count(&self) -> usize {
        self.live_samples
    }

    #[must_use]
    pub fn is_incomplete_badge_active(&self) -> bool {
        match &self.token_probe {
            TokenProbeState::IsToken(ov) => ov.is_incomplete(),
            _ => false,
        }
    }

    #[must_use]
    pub fn contract_overview(&self) -> Option<&ContractOverview> {
        self.contract_overview.as_ref()
    }

    #[must_use]
    pub fn source(&self) -> Option<&ContractSource> {
        self.source.as_ref()
    }

    /// Test helper: Events sub-tab row count once loaded.
    #[must_use]
    pub fn events_count(&self) -> Option<usize> {
        match self.events.as_ref() {
            Some(Ok(page)) => Some(page.logs.len()),
            _ => None,
        }
    }

    #[must_use]
    pub fn events_offset(&self) -> u32 {
        self.events_offset
    }

    #[must_use]
    pub fn events_window(&self) -> Option<(u64, u64)> {
        match self.events.as_ref() {
            Some(Ok(page)) => Some((page.window_from.value(), page.window_to.value())),
            _ => None,
        }
    }

    #[must_use]
    pub fn last_result_matches_uint(&self, expected: u128) -> bool {
        match self.last_result.as_ref() {
            Some(Ok(values)) if values.len() == 1 => {
                matches!(values[0], DecodedValue::Uint(v) if v == expected)
            }
            _ => false,
        }
    }

    #[must_use]
    pub fn last_result_is_error_containing(&self, needle: &str) -> bool {
        matches!(self.last_result.as_ref(), Some(Err(msg)) if msg.contains(needle))
    }

    #[must_use]
    pub fn storage_value_u128(&self) -> Option<u128> {
        let Some(Ok(word)) = self.storage_result.as_ref() else {
            return None;
        };
        if word[..16].iter().any(|b| *b != 0) {
            return None;
        }
        let mut buf = [0u8; 16];
        buf.copy_from_slice(&word[16..]);
        Some(u128::from_be_bytes(buf))
    }

    #[must_use]
    pub fn selected(&self) -> usize {
        self.active_list_state().selected().unwrap_or(0)
    }

    fn active_list_state(&self) -> &ListState {
        match self.active_tab_or_fallback() {
            AddressTab::Tokens => &self.token_list_state,
            AddressTab::Transfers => &self.transfers_list_state,
            AddressTab::Token if matches!(self.active_token_sub, TokenSubTab::Transfers) => {
                &self.token_transfers_list_state
            }
            _ => &self.tx_list_state,
        }
    }

    fn active_list_state_mut(&mut self) -> &mut ListState {
        match self.active_tab_or_fallback() {
            AddressTab::Tokens => &mut self.token_list_state,
            AddressTab::Transfers => &mut self.transfers_list_state,
            AddressTab::Token if matches!(self.active_token_sub, TokenSubTab::Transfers) => {
                &mut self.token_transfers_list_state
            }
            _ => &mut self.tx_list_state,
        }
    }

    fn active_list_len(&self) -> usize {
        match self.active_tab_or_fallback() {
            AddressTab::Transactions => self
                .account_tx_page
                .as_ref()
                .map(|p| p.txs.len())
                .unwrap_or(0),
            AddressTab::Transfers => self
                .transfers
                .as_ref()
                .map(|p| p.events.len())
                .unwrap_or(0),
            AddressTab::Tokens => self.holdings.as_ref().map(|h| h.len()).unwrap_or(0),
            AddressTab::Token if matches!(self.active_token_sub, TokenSubTab::Transfers) => self
                .token_transfers
                .as_ref()
                .map(|p| p.events.len())
                .unwrap_or(0),
            _ => 0,
        }
    }

    fn drain_feed(&mut self) {
        while let Ok(update) = self.feed.updates_rx.try_recv() {
            self.current = Some(update);
        }
        while let Ok(page) = self.feed.account_transactions_rx.try_recv() {
            self.account_tx_page = Some(page);
            self.clamp_account_tx_selection();
        }
        while let Ok(page) = self.feed.transfers_rx.try_recv() {
            self.transfers = Some(page);
            self.clamp_transfers_selection();
        }
        while let Ok(holdings) = self.feed.portfolio_rx.try_recv() {
            self.holdings = Some(holdings);
            self.clamp_token_selection();
        }
        while let Ok(opt_overview) = self.feed.token_overview_rx.try_recv() {
            self.token_probe = match opt_overview {
                Some(ov) => TokenProbeState::IsToken(ov),
                None => TokenProbeState::NotToken,
            };
        }
        while let Ok(lookup) = self.feed.token_price_rx.try_recv() {
            if let TokenProbeState::IsToken(ref mut ov) = self.token_probe {
                ov.price = lookup.clone();
            }
            if let PriceLookup::Available(ref p) = lookup {
                self.append_live_sample(p.clone());
            }
            self.token_price = lookup;
        }
        while let Ok(series) = self.feed.token_series_rx.try_recv() {
            self.token_series.insert(series.window, series);
        }
        while let Ok(page) = self.feed.token_transfers_rx.try_recv() {
            self.token_transfers = Some(page);
            self.clamp_token_transfers_selection();
        }
        while let Ok(ov) = self.feed.contract_overview_rx.try_recv() {
            self.contract_overview = Some(ov);
        }
        while let Ok(src) = self.feed.source_rx.try_recv() {
            self.functions = parse_abi_functions(&src.abi);
            self.functions.sort_by(|a, b| a.name.cmp(&b.name));
            self.source = Some(src);
            self.clamp_file_selection();
            self.reset_args_for_current_fn();
        }
        while let Ok(ov) = self.feed.contract_impl_overview_rx.try_recv() {
            self.contract_overview_impl = Some(ov);
        }
        while let Ok(src) = self.feed.source_impl_rx.try_recv() {
            self.functions_impl = parse_abi_functions(&src.abi);
            self.functions_impl.sort_by(|a, b| a.name.cmp(&b.name));
            self.source_impl = Some(src);
            self.clamp_file_selection_impl();
            if self.functions_impl.is_empty() {
                self.function_list_state_impl.select(None);
                self.arg_buffers_impl.clear();
            } else {
                self.function_list_state_impl.select(Some(0));
                let arg_count = self.functions_impl[0].inputs.len();
                self.arg_buffers_impl = vec![String::new(); arg_count];
            }
            self.arg_cursor_impl = 0;
            self.last_result_impl = None;
            self.last_result_for_impl = None;
        }
        while let Ok(delivery) = self.feed.read_rx.try_recv() {
            let mapped = delivery
                .result
                .map_err(|e| domain_error_message(&e));
            match delivery.calldata_source {
                ReadCalldataSource::ProxyArtifact => {
                    self.last_result = Some(mapped);
                    self.last_result_for = Some(delivery.signature);
                }
                ReadCalldataSource::ImplementationArtifact => {
                    self.last_result_impl = Some(mapped);
                    self.last_result_for_impl = Some(delivery.signature);
                }
            }
        }
        while let Ok(result) = self.feed.events_rx.try_recv() {
            if let Ok(page) = result.as_ref() {
                self.events_head = Some(page.head);
            }
            self.events = Some(result.map_err(|e| domain_error_message(&e)));
        }
        while let Ok(result) = self.feed.storage_rx.try_recv() {
            self.storage_result = Some(result.map_err(|e| domain_error_message(&e)));
            self.storage_value_line = 0;
            self.with_storage_value_scroll(|s| s.reset());
        }
    }

    fn append_live_sample(&mut self, price: TokenPrice) {
        let window = self.active_token_window;
        let entry = self
            .token_series
            .entry(window)
            .or_insert_with(|| PriceSeries::empty(window));
        entry.points.push(PricePoint {
            at: price.as_of,
            value: price.value,
        });
        let cap = PriceSeries::ROLLING_CAP;
        if entry.points.len() > cap {
            let excess = entry.points.len() - cap;
            entry.points.drain(..excess);
        }
        self.live_samples = self.live_samples.saturating_add(1);
    }

    fn clamp_account_tx_selection(&mut self) {
        let len = self
            .account_tx_page
            .as_ref()
            .map(|p| p.txs.len())
            .unwrap_or(0);
        clamp_selection(&mut self.tx_list_state, len);
    }

    fn clamp_transfers_selection(&mut self) {
        let len = self.transfers.as_ref().map(|p| p.events.len()).unwrap_or(0);
        clamp_selection(&mut self.transfers_list_state, len);
    }

    fn clamp_token_selection(&mut self) {
        let len = self.holdings.as_ref().map(|h| h.len()).unwrap_or(0);
        clamp_selection(&mut self.token_list_state, len);
    }

    fn clamp_token_transfers_selection(&mut self) {
        let len = self
            .token_transfers
            .as_ref()
            .map(|p| p.events.len())
            .unwrap_or(0);
        clamp_selection(&mut self.token_transfers_list_state, len);
    }

    fn clamp_file_selection(&mut self) {
        let len = self.source.as_ref().map(|s| s.files.len()).unwrap_or(0);
        clamp_selection(&mut self.file_list_state, len);
    }

    fn clamp_file_selection_impl(&mut self) {
        let len = self.source_impl.as_ref().map(|s| s.files.len()).unwrap_or(0);
        clamp_selection(&mut self.file_list_state_impl, len);
    }

    fn select_delta(&mut self, delta: i32) {
        let len = self.active_list_len();
        if len == 0 {
            return;
        }
        let state = self.active_list_state_mut();
        let current = state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1);
        state.select(Some(next as usize));
    }

    #[must_use]
    pub fn last_copied_value(&self) -> Option<&str> {
        self.last_copied_value.as_deref()
    }

    fn copy_address_hex(&mut self) {
        if self.current.is_none() {
            return;
        }
        let value = NavigableValue::Address(self.address);
        self.last_copied_value = Some(value.copy_text());
        if let Some(services) = self.cursor_services.as_ref() {
            services.copy(&value);
        }
    }

    fn copy_ens_or_address(&mut self) {
        let Some(ov) = self.current.as_ref() else {
            return;
        };
        let value = match ov.ens_name.as_deref() {
            Some(name) => NavigableValue::EnsName(name.to_string()),
            None => NavigableValue::Address(ov.address),
        };
        self.last_copied_value = Some(value.copy_text());
        if let Some(services) = self.cursor_services.as_ref() {
            services.copy(&value);
        }
    }

    fn copy_active_as_csv(&mut self) {
        if self.current.is_none() {
            return;
        }
        let csv = match self.active_tab_or_fallback() {
            AddressTab::Transactions => csv_for_account_tx(self.account_tx_page.as_ref()),
            AddressTab::Transfers => csv_for_transfers(self.transfers.as_ref()),
            AddressTab::Tokens => csv_for_holdings(self.holdings.as_ref()),
            AddressTab::Overview
            | AddressTab::Token
            | AddressTab::Contract
            | AddressTab::ContractImpl => csv_for_overview(self.current.as_ref()),
        };
        // CSV is opaque text (no navigation target); wrap it in
        // `Plain` so the clipboard path mirrors every other copy.
        let value = NavigableValue::Plain(csv);
        self.last_copied_value = Some(value.copy_text());
        if let Some(services) = self.cursor_services.as_ref() {
            services.copy(&value);
        }
    }

    // ------------------------------------------------------------------
    // Test-only seeding helpers. Production feeds populate these
    // fields via `drain_feed`; the setters let functional tests
    // bypass the async runtime.
    // ------------------------------------------------------------------

    #[doc(hidden)]
    pub fn set_overview_for_test(&mut self, overview: AddressOverview) {
        self.current = Some(overview);
    }

    #[doc(hidden)]
    pub fn set_transfers_for_test(&mut self, page: TransferPage) {
        self.transfers = Some(page);
        self.clamp_transfers_selection();
    }

    #[doc(hidden)]
    pub fn set_account_transactions_for_test(&mut self, page: AccountTxPage) {
        self.account_tx_page = Some(page);
        self.clamp_account_tx_selection();
    }

    #[doc(hidden)]
    pub fn set_holdings_for_test(&mut self, holdings: Vec<TokenHolding>) {
        self.holdings = Some(holdings);
        self.clamp_token_selection();
    }

    #[doc(hidden)]
    pub fn set_token_overview_for_test(&mut self, overview: Option<TokenOverview>) {
        self.token_probe = match overview {
            Some(ov) => TokenProbeState::IsToken(ov),
            None => TokenProbeState::NotToken,
        };
    }

    #[doc(hidden)]
    pub fn set_contract_source_for_test(&mut self, source: ContractSource) {
        self.functions = parse_abi_functions(&source.abi);
        self.functions.sort_by(|a, b| a.name.cmp(&b.name));
        self.source = Some(source);
        self.clamp_file_selection();
        self.reset_args_for_current_fn();
    }

    #[doc(hidden)]
    pub fn set_contract_overview_for_test(&mut self, overview: ContractOverview) {
        self.contract_overview = Some(overview);
    }

    #[doc(hidden)]
    pub fn set_contract_overview_impl_for_test(&mut self, overview: ContractOverview) {
        self.contract_overview_impl = Some(overview);
    }

    #[doc(hidden)]
    pub fn set_contract_source_impl_for_test(&mut self, source: ContractSource) {
        self.functions_impl = parse_abi_functions(&source.abi);
        self.functions_impl.sort_by(|a, b| a.name.cmp(&b.name));
        self.source_impl = Some(source);
        self.clamp_file_selection_impl();
        if self.functions_impl.is_empty() {
            self.function_list_state_impl.select(None);
            self.arg_buffers_impl.clear();
        } else {
            self.function_list_state_impl.select(Some(0));
            let arg_count = self.functions_impl[0].inputs.len();
            self.arg_buffers_impl = vec![String::new(); arg_count];
        }
        self.arg_cursor_impl = 0;
        self.last_result_impl = None;
        self.last_result_for_impl = None;
    }

    // ------------------------------------------------------------------
    // Contract Read helpers
    // ------------------------------------------------------------------

    fn selected_function(&self) -> Option<&AbiFunction> {
        let idx = if self.contract_main_is_impl() {
            self.function_list_state_impl.selected().unwrap_or(0)
        } else {
            self.function_list_state.selected().unwrap_or(0)
        };
        let fns = if self.contract_main_is_impl() {
            &self.functions_impl
        } else {
            &self.functions
        };
        fns.get(idx)
    }

    fn selected_file(&self) -> Option<&SourceFile> {
        let files = if self.contract_main_is_impl() {
            self.source_impl.as_ref().map(|s| &s.files)?
        } else {
            self.source.as_ref().map(|s| &s.files)?
        };
        let idx = if self.contract_main_is_impl() {
            self.file_list_state_impl.selected().unwrap_or(0)
        } else {
            self.file_list_state.selected().unwrap_or(0)
        };
        files.get(idx)
    }

    fn reset_args_for_current_fn(&mut self) {
        let arg_count = self
            .selected_function()
            .map(|f| f.inputs.len())
            .unwrap_or(0);
        if self.contract_main_is_impl() {
            self.arg_buffers_impl = vec![String::new(); arg_count];
            self.arg_cursor_impl = 0;
            self.last_result_impl = None;
            self.last_result_for_impl = None;
        } else {
            self.arg_buffers = vec![String::new(); arg_count];
            self.arg_cursor = 0;
            self.last_result = None;
            self.last_result_for = None;
        }
    }

    fn select_function_delta(&mut self, delta: i32) {
        let fns = if self.contract_main_is_impl() {
            &self.functions_impl
        } else {
            &self.functions
        };
        if fns.is_empty() {
            return;
        }
        let current = if self.contract_main_is_impl() {
            self.function_list_state_impl.selected().unwrap_or(0) as i32
        } else {
            self.function_list_state.selected().unwrap_or(0) as i32
        };
        let next = (current + delta).clamp(0, fns.len() as i32 - 1);
        if self.contract_main_is_impl() {
            self.function_list_state_impl.select(Some(next as usize));
        } else {
            self.function_list_state.select(Some(next as usize));
        }
        self.reset_args_for_current_fn();
    }

    fn build_args(&self) -> Result<Vec<AbiValue>, String> {
        let function = self
            .selected_function()
            .ok_or_else(|| "no function selected".to_string())?;
        let buffers = if self.contract_main_is_impl() {
            &self.arg_buffers_impl
        } else {
            &self.arg_buffers
        };
        if function.inputs.len() != buffers.len() {
            return Err(format!(
                "arg buffer mismatch ({} vs {})",
                function.inputs.len(),
                buffers.len()
            ));
        }
        let mut values = Vec::with_capacity(function.inputs.len());
        for (param, raw) in function.inputs.iter().zip(buffers.iter()) {
            let raw = raw.trim();
            let value = match &param.kind {
                AbiParamType::Address => Address::from_hex(raw)
                    .map(AbiValue::Address)
                    .map_err(|e| format!("{}: {e}", param.name))?,
                AbiParamType::Uint { .. } => {
                    let n = if let Some(hex) = raw.strip_prefix("0x") {
                        u128::from_str_radix(hex, 16)
                    } else {
                        raw.parse::<u128>()
                    }
                    .map_err(|e| format!("{}: invalid uint: {e}", param.name))?;
                    AbiValue::Uint(n)
                }
                AbiParamType::Bool => match raw.to_ascii_lowercase().as_str() {
                    "true" | "1" => AbiValue::Bool(true),
                    "false" | "0" => AbiValue::Bool(false),
                    other => {
                        return Err(format!("{}: expected true/false, got {other}", param.name));
                    }
                },
                AbiParamType::String => AbiValue::String(raw.to_string()),
                other => {
                    return Err(format!("{}: unsupported input type {other:?}", param.name));
                }
            };
            values.push(value);
        }
        Ok(values)
    }

    fn execute_current(&mut self) {
        let Some(function) = self.selected_function().cloned() else {
            return;
        };
        if !function.is_executable() {
            let err = Err("function has unsupported ABI input types".to_string());
            let sig = function.signature();
            if self.contract_main_is_impl() {
                self.last_result_impl = Some(err);
                self.last_result_for_impl = Some(sig);
            } else {
                self.last_result = Some(err);
                self.last_result_for = Some(sig);
            }
            return;
        }
        let calldata_source = if self.contract_main_is_impl() {
            ReadCalldataSource::ImplementationArtifact
        } else {
            ReadCalldataSource::ProxyArtifact
        };
        match self.build_args() {
            Ok(args) => {
                self.with_read_result_scroll(|s| s.reset());
                let _ = self.feed.read_tx.send(ReadRequest {
                    function,
                    args,
                    calldata_source,
                });
            }
            Err(msg) => {
                let sig = function.signature();
                if self.contract_main_is_impl() {
                    self.last_result_impl = Some(Err(msg));
                    self.last_result_for_impl = Some(sig);
                } else {
                    self.last_result = Some(Err(msg));
                    self.last_result_for = Some(sig);
                }
            }
        }
    }

    fn file_delta(&mut self, delta: i32) {
        let len = if self.contract_main_is_impl() {
            self.source_impl.as_ref().map(|s| s.files.len()).unwrap_or(0)
        } else {
            self.source.as_ref().map(|s| s.files.len()).unwrap_or(0)
        };
        if len == 0 {
            return;
        }
        let current = if self.contract_main_is_impl() {
            self.file_list_state_impl.selected().unwrap_or(0) as i32
        } else {
            self.file_list_state.selected().unwrap_or(0) as i32
        };
        let next = (current + delta).clamp(0, len as i32 - 1);
        if self.contract_main_is_impl() {
            self.file_list_state_impl.select(Some(next as usize));
        } else {
            self.file_list_state.select(Some(next as usize));
        }
        self.with_contract_scroll(|s| s.reset());
    }

    fn with_contract_scroll(&self, f: impl FnOnce(&mut ScrollState)) {
        let cell = if self.contract_main_is_impl() {
            &self.impl_contract_scroll
        } else {
            &self.contract_scroll
        };
        let mut s = cell.get();
        f(&mut s);
        cell.set(s);
    }

    fn bound_scroll_for(&self, body: &str, area: Rect) -> u16 {
        let content_lines = body.lines().count() as u16;
        let viewport = area.height.saturating_sub(2);
        self.with_contract_scroll(|s| s.set_dimensions(content_lines, viewport));
        self.contract_scroll.get().offset()
    }

    fn bound_read_result_for(&self, body: &str, area: Rect) -> u16 {
        let content_lines = body.lines().count() as u16;
        let viewport = area.height.saturating_sub(2);
        self.with_read_result_scroll(|s| s.set_dimensions(content_lines, viewport));
        self.read_result_scroll.get().offset()
    }

    fn bound_storage_value_for(&self, body: &str, area: Rect) -> u16 {
        let n = body.lines().count().max(1);
        let viewport = area.height.saturating_sub(2);
        let row = self.storage_value_line.min(n.saturating_sub(1)) as u16;
        self.with_storage_value_scroll(|s| {
            s.set_dimensions(n as u16, viewport);
            s.scroll_row_into_view(row);
        });
        self.storage_value_scroll.get().offset()
    }

    fn storage_value_body(&self) -> String {
        match (self.storage_slot_requested, self.storage_result.as_ref()) {
            (Some(slot), Some(Ok(word))) => format_storage_word(slot, word),
            (Some(_), Some(Err(msg))) => format!("ERROR: {msg}"),
            (Some(_), None) => "Reading...".to_string(),
            (None, None) => "(press Enter to read the current slot)".to_string(),
            (None, Some(Err(msg))) => format!("ERROR: {msg}"),
            (None, Some(Ok(_))) => unreachable!(),
        }
    }

    fn set_token_window(&mut self, window: PriceWindow) {
        if self.active_token_window == window {
            return;
        }
        self.active_token_window = window;
        if !self.token_series.contains_key(&window) {
            let _ = self.feed.token_window_req_tx.send(window);
        }
    }
}

// ---------------------------------------------------------------------------
// Screen impl
// ---------------------------------------------------------------------------

impl Screen for AddressDetailScreen {
    fn title(&self) -> &str {
        "Address"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let active = self.active_tab_or_fallback();
        let has_sub = matches!(
            active,
            AddressTab::Contract | AddressTab::ContractImpl | AddressTab::Token
        );

        let constraints: Vec<Constraint> = if has_sub {
            vec![
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(3),
            ]
        } else {
            vec![
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(3),
            ]
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        // Header.
        let header = match self.current.as_ref() {
            Some(ov) => {
                let ens = match ov.ens_name.as_deref() {
                    Some(n) => format!(" ({n})"),
                    None => String::new(),
                };
                let delegation = match ov.delegated_to {
                    Some(delegate) => format!("  delegated to {}", delegate.to_hex()),
                    None => String::new(),
                };
                format!(
                    "Address {addr}{ens}{delegation}",
                    addr = ov.address.to_hex(),
                )
            }
            None => "Address (loading...)".to_string(),
        };
        frame.render_widget(
            Paragraph::new(header).block(Block::default().borders(Borders::ALL).title("Address")),
            chunks[0],
        );

        let palette = PalettePreset::DarkDefault.palette();
        let sub_visible = has_sub;
        let main_border = tab_strip_border_style(
            self.focus_layer,
            DetailTabStrip::Main,
            sub_visible,
            &palette,
        );
        let main_hi = tab_strip_highlight_style(
            self.focus_layer,
            DetailTabStrip::Main,
            sub_visible,
            &palette,
        );

        // Main tabs bar.
        let tabs_visible = self.visible_tabs();
        let titles: Vec<Line<'static>> = tabs_visible
            .iter()
            .map(|t| Line::from(format!(" {} ", t.label())))
            .collect();
        let active_idx = tabs_visible.iter().position(|&t| t == active).unwrap_or(0);
        frame.render_widget(
            Tabs::new(titles)
                .select(active_idx)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(main_border)
                        .title("Main tabs  —  Tab / ←→ · ↓ sub/body"),
                )
                .divider(" ")
                .highlight_style(main_hi),
            chunks[1],
        );

        let body_rect = if has_sub {
            // Render sub-tabs row.
            let sub_border =
                tab_strip_border_style(self.focus_layer, DetailTabStrip::Sub, true, &palette);
            let sub_hi =
                tab_strip_highlight_style(self.focus_layer, DetailTabStrip::Sub, true, &palette);
            match active {
                AddressTab::Contract | AddressTab::ContractImpl => {
                    let sub_title = if matches!(active, AddressTab::ContractImpl) {
                        "Impl sub-tabs  —  [ / ] · ←→"
                    } else {
                        "Contract sub-tabs  —  [ / ] · ←→"
                    };
                    let sub_titles: Vec<Line<'static>> = ContractSubTab::ALL
                        .iter()
                        .map(|t| Line::from(format!(" {} ", t.label())))
                        .collect();
                    frame.render_widget(
                        Tabs::new(sub_titles)
                            .select(self.active_contract_sub.index())
                            .block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .border_style(sub_border)
                                    .title(sub_title),
                            )
                            .divider(" ")
                            .highlight_style(sub_hi),
                        chunks[2],
                    );
                }
                AddressTab::Token => {
                    let sub_titles: Vec<Line<'static>> = TokenSubTab::ALL
                        .iter()
                        .map(|t| Line::from(format!(" {} ", t.label())))
                        .collect();
                    frame.render_widget(
                        Tabs::new(sub_titles)
                            .select(self.active_token_sub.index())
                            .block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .border_style(sub_border)
                                    .title("Token sub-tabs  —  [ / ] · ←→"),
                            )
                            .divider(" ")
                            .highlight_style(sub_hi),
                        chunks[2],
                    );
                }
                _ => {}
            }
            chunks[3]
        } else {
            chunks[2]
        };

        // Body.
        match active {
            AddressTab::Overview => self.render_overview(frame, body_rect),
            AddressTab::Transactions => self.render_transactions(frame, body_rect),
            AddressTab::Transfers => self.render_transfers(frame, body_rect),
            AddressTab::Tokens => self.render_tokens(frame, body_rect),
            AddressTab::Token => match self.active_token_sub {
                TokenSubTab::Overview => self.render_token_overview(frame, body_rect),
                TokenSubTab::Transfers => self.render_token_transfers(frame, body_rect),
                TokenSubTab::Chart => self.render_token_chart(frame, body_rect),
            },
            AddressTab::Contract => match self.active_contract_sub {
                ContractSubTab::Overview => self.render_contract_overview(frame, body_rect),
                ContractSubTab::Source => self.render_source_tab(frame, body_rect),
                ContractSubTab::Abi => self.render_abi_tab(frame, body_rect),
                ContractSubTab::Read => self.render_read_tab(frame, body_rect),
                ContractSubTab::Events => self.render_events_tab(frame, body_rect),
                ContractSubTab::Storage => self.render_storage_tab(frame, body_rect),
            },
            AddressTab::ContractImpl => match self.active_contract_sub {
                ContractSubTab::Overview => self.render_contract_overview_impl(frame, body_rect),
                ContractSubTab::Source => self.render_source_tab(frame, body_rect),
                ContractSubTab::Abi => self.render_abi_tab(frame, body_rect),
                ContractSubTab::Read => self.render_read_tab(frame, body_rect),
                ContractSubTab::Events => self.render_events_tab(frame, body_rect),
                ContractSubTab::Storage => self.render_storage_tab(frame, body_rect),
            },
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        let cmd = self.dispatch_key(key);
        self.scroll = self.scroll.min(self.scroll_cap.get());
        cmd
    }

    fn tick(&mut self) -> Command {
        self.drain_feed();
        // Fire the first Events request lazily once the user enters
        // the Events sub-tab, so we never issue eth_getLogs for
        // contracts the user just glances at.
        if self.contract_strip_like_contract()
            && matches!(self.active_contract_sub, ContractSubTab::Events)
            && !self.events_requested
        {
            let _ = self.feed.events_tx.send(EventsRequest {
                head_hint: self.events_head,
                offset: self.events_offset,
            });
            self.events_requested = true;
        }
        Command::None
    }

    fn footer_hints(&self) -> Vec<(&'static str, &'static str)> {
        let mut hints: Vec<(&'static str, &'static str)> = vec![
            ("Tab", "Tabs / subs"),
            ("←/→", "Tab row"),
            ("↑/↓", "Focus"),
            ("Enter", "Open"),
            ("y", "Copy"),
            ("Y", "Canonical"),
            ("e", "Export"),
        ];
        match self.active_tab_or_fallback() {
            AddressTab::Contract | AddressTab::ContractImpl | AddressTab::Token => {
                hints.push(("[", "Prev sub"));
                hints.push(("]", "Next sub"));
            }
            _ => {}
        }
        if matches!(self.active_tab, AddressTab::Token)
            && matches!(self.active_token_sub, TokenSubTab::Chart)
        {
            hints.push(("1..3", "Window"));
        }
        if matches!(
            self.active_tab,
            AddressTab::Contract | AddressTab::ContractImpl
        ) && matches!(self.active_contract_sub, ContractSubTab::Abi)
        {
            hints.push(("Y", "Copy ABI"));
        }
        hints
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl AddressDetailScreen {
    fn body_outline(&self) -> Style {
        let palette = PalettePreset::DarkDefault.palette();
        detail_body_border_style(self.focus_layer, &palette)
    }

    /// Border for a split pane when the body has content focus: only
    /// `focused` pane gets the accent outline.
    fn split_pane_border(&self, focused: bool) -> Style {
        let palette = PalettePreset::DarkDefault.palette();
        Self::pane_border(self.focus_layer, focused, &palette)
    }

    fn pane_border(
        focus_layer: DetailFocusLayer,
        this_pane_focused: bool,
        palette: &Palette,
    ) -> Style {
        if focus_layer == DetailFocusLayer::Content && this_pane_focused {
            detail_body_border_style(DetailFocusLayer::Content, palette)
        } else {
            Style::default().fg(palette.foreground)
        }
    }

    fn reset_contract_subtab_ui_state(&mut self) {
        self.storage_focus = StorageFocus::Slot;
        self.with_storage_value_scroll(|s| s.reset());
        if self.contract_main_is_impl() {
            self.source_focus_impl = SourceFocus::Files;
            self.read_focus_impl = ReadFocus::FunctionList;
        } else {
            self.source_focus = SourceFocus::Files;
            self.read_focus = ReadFocus::FunctionList;
        }
        self.with_contract_scroll(|s| s.reset());
        self.with_read_result_scroll(|s| s.reset());
    }

    fn with_read_result_scroll(&self, f: impl FnOnce(&mut ScrollState)) {
        let cell = if self.contract_main_is_impl() {
            &self.read_result_scroll_impl
        } else {
            &self.read_result_scroll
        };
        let mut s = cell.get();
        f(&mut s);
        cell.set(s);
    }

    fn with_storage_value_scroll(&self, f: impl FnOnce(&mut ScrollState)) {
        let mut s = self.storage_value_scroll.get();
        f(&mut s);
        self.storage_value_scroll.set(s);
    }

    const fn main_tab_exposes_sub_strip(active: AddressTab) -> bool {
        matches!(
            active,
            AddressTab::Contract | AddressTab::ContractImpl | AddressTab::Token
        )
    }

    fn promote_focus_up_from_list(&mut self) {
        self.focus_layer = match self.active_tab_or_fallback() {
            AddressTab::Token
                if matches!(
                    self.active_token_sub,
                    TokenSubTab::Transfers | TokenSubTab::Overview | TokenSubTab::Chart
                ) =>
            {
                DetailFocusLayer::Subtabs
            }
            _ => DetailFocusLayer::MainTabs,
        };
    }

    fn dispatch_key(&mut self, key: KeyEvent) -> Command {
        let is_back_tab = key.code == KeyCode::BackTab
            || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT));

        // Global keys first.
        if matches!(key.code, KeyCode::Char('q')) {
            return Command::Quit;
        }
        if matches!(key.code, KeyCode::Esc) {
            return Command::Pop;
        }
        // Backspace deactivates the field cursor on the Overview
        // tab without popping the screen. Moved off Esc so Esc
        // always pops (plan/15-backlog.md §8.16). Other tabs that
        // use Backspace for text input (Read/args, Storage/slot) are
        // dispatched further down; this arm only fires on Overview.
        let contract_overview_cursor = matches!(
            self.active_tab,
            AddressTab::Contract | AddressTab::ContractImpl
        ) && matches!(self.active_contract_sub, ContractSubTab::Overview);
        if matches!(key.code, KeyCode::Backspace)
            && (matches!(self.active_tab_or_fallback(), AddressTab::Overview)
                || contract_overview_cursor)
            && self.cursor.is_active()
        {
            self.cursor.deactivate();
            return Command::None;
        }

        // Clipboard bindings (available from any tab). When the
        // cursor is live on Overview, `y` copies the current field's
        // canonical form and also pushes through the real clipboard
        // adapter. Falls back to the legacy "copy the address" flow
        // before the user activates the cursor.
        match key.code {
            KeyCode::Char('y') => {
                if matches!(
                    self.active_tab,
                    AddressTab::Contract | AddressTab::ContractImpl
                ) && matches!(self.active_contract_sub, ContractSubTab::Storage)
                    && self.storage_focus == StorageFocus::Value
                {
                    let body = self.storage_value_body();
                    let lines: Vec<&str> = body.lines().collect();
                    let n = lines.len().max(1);
                    let idx = self.storage_value_line.min(n.saturating_sub(1));
                    if let Some(line) = lines.get(idx) {
                        let text = (*line).to_string();
                        self.last_copied_value = Some(text.clone());
                        if let Some(services) = self.cursor_services.as_ref() {
                            services.copy(&NavigableValue::Plain(text));
                        }
                    }
                    return Command::None;
                }
                let cursor_tab = matches!(self.active_tab_or_fallback(), AddressTab::Overview)
                    || (matches!(
                        self.active_tab,
                        AddressTab::Contract | AddressTab::ContractImpl
                    ) && matches!(self.active_contract_sub, ContractSubTab::Overview));
                if cursor_tab && self.cursor.is_active() {
                    let fields = self.navigable_fields();
                    if let Some(entry) = self.cursor.current(&fields) {
                        self.last_copied_value = Some(entry.value.copy_text());
                        if let Some(services) = self.cursor_services.as_ref() {
                            services.copy(&entry.value);
                        }
                    }
                } else {
                    self.copy_address_hex();
                }
                return Command::None;
            }
            KeyCode::Char('Y') => {
                if matches!(
                    self.active_tab,
                    AddressTab::Contract | AddressTab::ContractImpl
                ) && matches!(self.active_contract_sub, ContractSubTab::Abi)
                {
                    self.copy_abi_to_clipboard();
                    return Command::None;
                }
                self.copy_ens_or_address();
                return Command::None;
            }
            KeyCode::Char('e') => {
                self.copy_active_as_csv();
                return Command::None;
            }
            _ => {}
        }

        // Token chart window: `1` / `2` / `3` only map to
        // `PriceWindow::{D1, M1, Y1}` when the Chart sub-tab is
        // actually in view. Digits are no longer consumed as
        // sub-tab selectors — `[` / `]` cycle sub-tabs instead (see
        // plan/15-backlog.md §8.16). The Read/args editor keeps
        // ownership of the digits whenever that focus is active.
        if self.focus_layer == DetailFocusLayer::Content
            && matches!(self.active_tab, AddressTab::Token)
            && matches!(self.active_token_sub, TokenSubTab::Chart)
        {
            match key.code {
                KeyCode::Char('1') => {
                    self.set_token_window(PriceWindow::D1);
                    return Command::None;
                }
                KeyCode::Char('2') => {
                    self.set_token_window(PriceWindow::M1);
                    return Command::None;
                }
                KeyCode::Char('3') => {
                    self.set_token_window(PriceWindow::Y1);
                    return Command::None;
                }
                _ => {}
            }
        }

        // Incomplete-token shortcut from any tab: `c` jumps to the
        // Contract sub-tab. See plan/8 §13.2.
        if matches!(key.code, KeyCode::Char('c')) && self.is_incomplete_badge_active() {
            self.active_tab = AddressTab::Contract;
            self.active_contract_sub = ContractSubTab::Overview;
            self.focus_layer = DetailFocusLayer::Content;
            self.reset_contract_subtab_ui_state();
            return Command::None;
        }

        // Main tab / sub-tab cycling on Tab keys.
        if is_back_tab {
            if self.focus_layer == DetailFocusLayer::Subtabs {
                match self.active_tab_or_fallback() {
                    AddressTab::Contract | AddressTab::ContractImpl => {
                        self.active_contract_sub = self.active_contract_sub.previous();
                        self.reset_contract_subtab_ui_state();
                        return Command::None;
                    }
                    AddressTab::Token => {
                        self.active_token_sub = self.active_token_sub.previous();
                        self.scroll = 0;
                        return Command::None;
                    }
                    _ => {}
                }
            }
            self.active_tab = self.prev_tab();
            self.scroll = 0;
            return Command::None;
        }
        if key.code == KeyCode::Tab {
            if self.focus_layer == DetailFocusLayer::Subtabs {
                match self.active_tab_or_fallback() {
                    AddressTab::Contract | AddressTab::ContractImpl => {
                        self.active_contract_sub = self.active_contract_sub.next();
                        self.reset_contract_subtab_ui_state();
                        return Command::None;
                    }
                    AddressTab::Token => {
                        self.active_token_sub = self.active_token_sub.next();
                        self.scroll = 0;
                        return Command::None;
                    }
                    _ => {}
                }
            }
            self.active_tab = self.next_tab();
            self.scroll = 0;
            return Command::None;
        }

        if self.focus_layer == DetailFocusLayer::MainTabs {
            match key.code {
                KeyCode::Left => {
                    self.active_tab = self.prev_tab();
                    self.scroll = 0;
                    return Command::None;
                }
                KeyCode::Right => {
                    self.active_tab = self.next_tab();
                    self.scroll = 0;
                    return Command::None;
                }
                KeyCode::Down => {
                    let active = self.active_tab_or_fallback();
                    self.focus_layer = if Self::main_tab_exposes_sub_strip(active) {
                        DetailFocusLayer::Subtabs
                    } else {
                        DetailFocusLayer::Content
                    };
                    return Command::None;
                }
                KeyCode::Up => return Command::None,
                _ => {}
            }
        }

        if self.focus_layer == DetailFocusLayer::Subtabs {
            match key.code {
                KeyCode::Left => {
                    match self.active_tab_or_fallback() {
                        AddressTab::Contract | AddressTab::ContractImpl => {
                            self.active_contract_sub = self.active_contract_sub.previous();
                            self.reset_contract_subtab_ui_state();
                        }
                        AddressTab::Token => {
                            self.active_token_sub = self.active_token_sub.previous();
                            self.scroll = 0;
                        }
                        _ => {}
                    }
                    return Command::None;
                }
                KeyCode::Right => {
                    match self.active_tab_or_fallback() {
                        AddressTab::Contract | AddressTab::ContractImpl => {
                            self.active_contract_sub = self.active_contract_sub.next();
                            self.reset_contract_subtab_ui_state();
                        }
                        AddressTab::Token => {
                            self.active_token_sub = self.active_token_sub.next();
                            self.scroll = 0;
                        }
                        _ => {}
                    }
                    return Command::None;
                }
                KeyCode::Down => {
                    self.focus_layer = DetailFocusLayer::Content;
                    return Command::None;
                }
                KeyCode::Up => {
                    self.focus_layer = DetailFocusLayer::MainTabs;
                    return Command::None;
                }
                _ => {}
            }
        }

        // Sub-tab cycling.
        if matches!(key.code, KeyCode::Char(']')) {
            match self.active_tab_or_fallback() {
                AddressTab::Contract | AddressTab::ContractImpl => {
                    self.active_contract_sub = self.active_contract_sub.next();
                    self.reset_contract_subtab_ui_state();
                    return Command::None;
                }
                AddressTab::Token => {
                    self.active_token_sub = self.active_token_sub.next();
                    self.scroll = 0;
                    return Command::None;
                }
                _ => {}
            }
        }
        if matches!(key.code, KeyCode::Char('[')) {
            match self.active_tab_or_fallback() {
                AddressTab::Contract | AddressTab::ContractImpl => {
                    self.active_contract_sub = self.active_contract_sub.previous();
                    self.reset_contract_subtab_ui_state();
                    return Command::None;
                }
                AddressTab::Token => {
                    self.active_token_sub = self.active_token_sub.previous();
                    self.scroll = 0;
                    return Command::None;
                }
                _ => {}
            }
        }

        if self.focus_layer == DetailFocusLayer::Content {
            return match self.active_tab_or_fallback() {
                AddressTab::Overview => self.handle_overview_key(key),
                AddressTab::Transactions
                | AddressTab::Transfers
                | AddressTab::Tokens => self.handle_list_key(key),
                AddressTab::Token => self.handle_token_key(key),
                AddressTab::Contract | AddressTab::ContractImpl => self.handle_contract_key(key),
            };
        }
        Command::None
    }

    fn handle_overview_key(&mut self, key: KeyEvent) -> Command {
        // Arrow keys drive the field cursor. `k`/`j` keep the
        // historical "scroll by one line" behaviour so the Overview
        // text remains scrollable on narrow terminals.
        let fields = self.navigable_fields();
        match key.code {
            KeyCode::Up if !self.cursor.is_active() && self.scroll == 0 => {
                self.focus_layer = DetailFocusLayer::MainTabs;
                return Command::None;
            }
            KeyCode::Up if self.cursor.active() == Some(0) => {
                self.focus_layer = DetailFocusLayer::MainTabs;
                return Command::None;
            }
            KeyCode::Left => {
                self.cursor.move_in(fields.len(), CursorDir::Left);
                return Command::None;
            }
            KeyCode::Right => {
                self.cursor.move_in(fields.len(), CursorDir::Right);
                return Command::None;
            }
            KeyCode::Up => {
                self.cursor.move_in(fields.len(), CursorDir::Up);
                return Command::None;
            }
            KeyCode::Down => {
                self.cursor.move_in(fields.len(), CursorDir::Down);
                return Command::None;
            }
            KeyCode::Enter if self.cursor.is_active() => {
                if let (Some(entry), Some(services)) =
                    (self.cursor.current(&fields), self.cursor_services.as_ref())
                    && let Some(screen) = services.open(&entry.value)
                {
                    return Command::Push(screen);
                }
                return Command::None;
            }
            KeyCode::Char('k') => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            KeyCode::Char('j') => {
                self.scroll = self.scroll.saturating_add(1);
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_add(10);
            }
            KeyCode::Home => {
                self.scroll = 0;
            }
            _ => {}
        }
        Command::None
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Command {
        if matches!(key.code, KeyCode::Up) && self.selected() == 0 {
            self.promote_focus_up_from_list();
            return Command::None;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.select_delta(-1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.select_delta(1);
            }
            KeyCode::PageUp => {
                self.select_delta(-10);
            }
            KeyCode::PageDown => {
                self.select_delta(10);
            }
            KeyCode::Home => {
                self.active_list_state_mut().select(Some(0));
            }
            KeyCode::End => {
                let len = self.active_list_len();
                if len > 0 {
                    self.active_list_state_mut().select(Some(len - 1));
                }
            }
            KeyCode::Enter => {
                return self.enter_on_active_list();
            }
            _ => {}
        }
        Command::None
    }

    fn enter_on_active_list(&self) -> Command {
        match self.active_tab_or_fallback() {
            AddressTab::Transactions => {
                let hash = self
                    .account_tx_page
                    .as_ref()
                    .and_then(|p| p.txs.get(self.selected()))
                    .map(|t| t.tx_hash);
                match (hash, self.open_tx.as_ref()) {
                    (Some(hash), Some(factory)) => Command::Push(factory(hash)),
                    _ => Command::None,
                }
            }
            AddressTab::Transfers => {
                let hash = self
                    .transfers
                    .as_ref()
                    .and_then(|p| p.events.get(self.selected()))
                    .map(|e| e.tx_hash);
                match (hash, self.open_tx.as_ref()) {
                    (Some(hash), Some(factory)) => Command::Push(factory(hash)),
                    _ => Command::None,
                }
            }
            AddressTab::Tokens => {
                let contract = self
                    .holdings
                    .as_ref()
                    .and_then(|h| h.get(self.selected()))
                    .map(|h| h.metadata.address);
                match (contract, self.open_token.as_ref()) {
                    (Some(addr), Some(factory)) => Command::Push(factory(addr)),
                    _ => Command::None,
                }
            }
            _ => Command::None,
        }
    }

    fn handle_token_key(&mut self, key: KeyEvent) -> Command {
        match self.active_token_sub {
            TokenSubTab::Transfers => match key.code {
                KeyCode::Up if self.selected() == 0 => {
                    self.promote_focus_up_from_list();
                    Command::None
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.select_delta(-1);
                    Command::None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.select_delta(1);
                    Command::None
                }
                KeyCode::PageUp => {
                    self.select_delta(-10);
                    Command::None
                }
                KeyCode::PageDown => {
                    self.select_delta(10);
                    Command::None
                }
                KeyCode::Home => {
                    self.active_list_state_mut().select(Some(0));
                    Command::None
                }
                KeyCode::End => {
                    let len = self.active_list_len();
                    if len > 0 {
                        self.active_list_state_mut().select(Some(len - 1));
                    }
                    Command::None
                }
                KeyCode::Enter => {
                    let hash = self
                        .token_transfers
                        .as_ref()
                        .and_then(|p| p.events.get(self.selected()))
                        .map(|e| e.tx_hash);
                    match (hash, self.open_tx.as_ref()) {
                        (Some(hash), Some(factory)) => Command::Push(factory(hash)),
                        _ => Command::None,
                    }
                }
                _ => Command::None,
            },
            TokenSubTab::Overview => match key.code {
                KeyCode::Up if self.scroll == 0 => {
                    self.focus_layer = DetailFocusLayer::Subtabs;
                    Command::None
                }
                KeyCode::Up => {
                    self.scroll = self.scroll.saturating_sub(1);
                    Command::None
                }
                KeyCode::Down => {
                    self.scroll = self.scroll.saturating_add(1);
                    Command::None
                }
                KeyCode::Char('k') => {
                    self.scroll = self.scroll.saturating_sub(1);
                    Command::None
                }
                KeyCode::Char('j') => {
                    self.scroll = self.scroll.saturating_add(1);
                    Command::None
                }
                KeyCode::PageUp => {
                    self.scroll = self.scroll.saturating_sub(10);
                    Command::None
                }
                KeyCode::PageDown => {
                    self.scroll = self.scroll.saturating_add(10);
                    Command::None
                }
                KeyCode::Home => {
                    self.scroll = 0;
                    Command::None
                }
                _ => Command::None,
            },
            TokenSubTab::Chart => match key.code {
                KeyCode::Up => {
                    self.focus_layer = DetailFocusLayer::Subtabs;
                    Command::None
                }
                _ => Command::None,
            },
        }
    }

    fn handle_contract_key(&mut self, key: KeyEvent) -> Command {
        match self.active_contract_sub {
            ContractSubTab::Overview => self.handle_contract_overview_key(key),
            ContractSubTab::Source => self.handle_source_key(key),
            ContractSubTab::Abi => self.handle_abi_scroll_key(key),
            ContractSubTab::Read => self.handle_read_key(key),
            ContractSubTab::Events => self.handle_events_key(key),
            ContractSubTab::Storage => self.handle_storage_key(key),
        }
    }

    fn handle_contract_overview_key(&mut self, key: KeyEvent) -> Command {
        let fields = self.navigable_fields();
        let n = fields.len();
        match key.code {
            KeyCode::Up if !self.cursor.is_active() => {
                self.focus_layer = DetailFocusLayer::Subtabs;
                return Command::None;
            }
            KeyCode::Up if self.cursor.active() == Some(0) => {
                self.focus_layer = DetailFocusLayer::Subtabs;
                return Command::None;
            }
            KeyCode::Left => {
                self.cursor.move_in(n, CursorDir::Left);
                return Command::None;
            }
            KeyCode::Right => {
                self.cursor.move_in(n, CursorDir::Right);
                return Command::None;
            }
            KeyCode::Up => {
                self.cursor.move_in(n, CursorDir::Up);
                return Command::None;
            }
            KeyCode::Down => {
                self.cursor.move_in(n, CursorDir::Down);
                return Command::None;
            }
            KeyCode::Enter if self.cursor.is_active() => {
                if let (Some(entry), Some(services)) =
                    (self.cursor.current(&fields), self.cursor_services.as_ref())
                    && let Some(screen) = services.open(&entry.value)
                {
                    return Command::Push(screen);
                }
                return Command::None;
            }
            _ => {}
        }
        self.handle_paragraph_scroll_key(key)
    }

    fn copy_abi_to_clipboard(&mut self) {
        let source = if self.contract_main_is_impl() {
            self.source_impl.as_ref()
        } else {
            self.source.as_ref()
        };
        let Some(source) = source else {
            return;
        };
        if !source.is_verified || source.abi.trim().is_empty() {
            return;
        }
        let pretty = serde_json::from_str::<serde_json::Value>(&source.abi)
            .ok()
            .and_then(|v| serde_json::to_string_pretty(&v).ok())
            .unwrap_or_else(|| source.abi.clone());
        self.last_copied_value = Some(pretty.clone());
        if let Some(services) = self.cursor_services.as_ref() {
            services.copy(&NavigableValue::Plain(pretty));
        }
    }

    fn handle_paragraph_scroll_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.with_contract_scroll(|s| {
                    s.scroll_by(-1);
                });
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.with_contract_scroll(|s| {
                    s.scroll_by(1);
                });
            }
            KeyCode::PageUp => {
                self.with_contract_scroll(|s| {
                    s.page_up();
                });
            }
            KeyCode::PageDown => {
                self.with_contract_scroll(|s| {
                    s.page_down();
                });
            }
            KeyCode::Home => {
                self.with_contract_scroll(|s| {
                    s.home();
                });
            }
            KeyCode::End => {
                self.with_contract_scroll(|s| {
                    s.end();
                });
            }
            _ => {}
        }
        Command::None
    }

    /// ABI sub-tab scrolls only with `j` / `k` (no arrow / page keys).
    fn handle_abi_scroll_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Char('k') => {
                self.with_contract_scroll(|s| {
                    s.scroll_by(-1);
                });
            }
            KeyCode::Char('j') => {
                self.with_contract_scroll(|s| {
                    s.scroll_by(1);
                });
            }
            _ => {}
        }
        Command::None
    }

    fn handle_source_key(&mut self, key: KeyEvent) -> Command {
        let source = if self.contract_main_is_impl() {
            self.source_impl.as_ref()
        } else {
            self.source.as_ref()
        };
        let Some(source) = source else {
            return Command::None;
        };
        if !source.is_verified || source.files.is_empty() {
            return self.handle_paragraph_scroll_key(key);
        }
        match self.source_focus_active() {
            SourceFocus::Files => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    let sel = if self.contract_main_is_impl() {
                        self.file_list_state_impl.selected().unwrap_or(0)
                    } else {
                        self.file_list_state.selected().unwrap_or(0)
                    };
                    if sel == 0 {
                        self.focus_layer = DetailFocusLayer::Subtabs;
                    } else {
                        self.file_delta(-1);
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => self.file_delta(1),
                KeyCode::Right | KeyCode::Enter => {
                    self.set_source_focus_active(SourceFocus::Viewer);
                }
                _ => {}
            },
            SourceFocus::Viewer => match key.code {
                KeyCode::Left => {
                    self.set_source_focus_active(SourceFocus::Files);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let off = if self.contract_main_is_impl() {
                        self.impl_contract_scroll.get().offset()
                    } else {
                        self.contract_scroll.get().offset()
                    };
                    if off == 0 {
                        self.set_source_focus_active(SourceFocus::Files);
                    } else {
                        self.with_contract_scroll(|s| {
                            s.scroll_by(-1);
                        });
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.with_contract_scroll(|s| {
                        s.scroll_by(1);
                    });
                }
                KeyCode::PageUp => {
                    self.with_contract_scroll(|s| {
                        s.page_up();
                    });
                }
                KeyCode::PageDown => {
                    self.with_contract_scroll(|s| {
                        s.page_down();
                    });
                }
                KeyCode::Home => {
                    self.with_contract_scroll(|s| {
                        s.home();
                    });
                }
                KeyCode::End => {
                    self.with_contract_scroll(|s| {
                        s.end();
                    });
                }
                _ => {}
            },
        }
        Command::None
    }

    fn handle_read_key(&mut self, key: KeyEvent) -> Command {
        match self.read_focus_active() {
            ReadFocus::FunctionList => self.handle_read_list_key(key),
            ReadFocus::Args => self.handle_read_args_key(key),
            ReadFocus::Result => self.handle_read_result_key(key),
        }
    }

    fn handle_read_list_key(&mut self, key: KeyEvent) -> Command {
        let fns = if self.contract_main_is_impl() {
            &self.functions_impl
        } else {
            &self.functions
        };
        if fns.is_empty() {
            return Command::None;
        }
        let current = if self.contract_main_is_impl() {
            self.function_list_state_impl.selected().unwrap_or(0)
        } else {
            self.function_list_state.selected().unwrap_or(0)
        };
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if current == 0 {
                    self.focus_layer = DetailFocusLayer::Subtabs;
                } else {
                    self.select_function_delta(-1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => self.select_function_delta(1),
            KeyCode::PageUp => self.select_function_delta(-10),
            KeyCode::PageDown => self.select_function_delta(10),
            KeyCode::Home => {
                if self.contract_main_is_impl() {
                    self.function_list_state_impl.select(Some(0));
                } else {
                    self.function_list_state.select(Some(0));
                }
                self.reset_args_for_current_fn();
            }
            KeyCode::End => {
                let last = fns.len() - 1;
                if self.contract_main_is_impl() {
                    self.function_list_state_impl.select(Some(last));
                } else {
                    self.function_list_state.select(Some(last));
                }
                self.reset_args_for_current_fn();
            }
            KeyCode::Enter | KeyCode::Right => {
                if let Some(f) = self.selected_function() {
                    if f.inputs.is_empty() {
                        self.execute_current();
                    } else {
                        self.set_read_focus_active(ReadFocus::Args);
                        if self.contract_main_is_impl() {
                            self.arg_cursor_impl = 0;
                        } else {
                            self.arg_cursor = 0;
                        }
                    }
                }
            }
            _ => {}
        }
        Command::None
    }

    fn handle_read_args_key(&mut self, key: KeyEvent) -> Command {
        let n_args = if self.contract_main_is_impl() {
            self.arg_buffers_impl.len()
        } else {
            self.arg_buffers.len()
        };
        let arg_cursor = if self.contract_main_is_impl() {
            self.arg_cursor_impl
        } else {
            self.arg_cursor
        };
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if arg_cursor > 0 {
                    if self.contract_main_is_impl() {
                        self.arg_cursor_impl -= 1;
                    } else {
                        self.arg_cursor -= 1;
                    }
                } else {
                    self.set_read_focus_active(ReadFocus::FunctionList);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if arg_cursor + 1 < n_args {
                    if self.contract_main_is_impl() {
                        self.arg_cursor_impl += 1;
                    } else {
                        self.arg_cursor += 1;
                    }
                } else {
                    self.set_read_focus_active(ReadFocus::Result);
                    self.with_read_result_scroll(|s| s.reset());
                }
            }
            KeyCode::Tab => {
                self.set_read_focus_active(ReadFocus::Result);
                self.with_read_result_scroll(|s| s.reset());
            }
            KeyCode::Left => {
                self.set_read_focus_active(ReadFocus::FunctionList);
            }
            KeyCode::Backspace => {
                if self.contract_main_is_impl() {
                    if let Some(buf) = self.arg_buffers_impl.get_mut(self.arg_cursor_impl) {
                        buf.pop();
                    }
                } else if let Some(buf) = self.arg_buffers.get_mut(self.arg_cursor) {
                    buf.pop();
                }
            }
            KeyCode::Char(c) => {
                if self.contract_main_is_impl() {
                    if let Some(buf) = self.arg_buffers_impl.get_mut(self.arg_cursor_impl) {
                        buf.push(c);
                    }
                } else if let Some(buf) = self.arg_buffers.get_mut(self.arg_cursor) {
                    buf.push(c);
                }
            }
            KeyCode::Enter => {
                self.execute_current();
            }
            _ => {}
        }
        Command::None
    }

    fn handle_read_result_key(&mut self, key: KeyEvent) -> Command {
        let n_bufs = if self.contract_main_is_impl() {
            self.arg_buffers_impl.len()
        } else {
            self.arg_buffers.len()
        };
        let result_scroll_off = if self.contract_main_is_impl() {
            self.read_result_scroll_impl.get().offset()
        } else {
            self.read_result_scroll.get().offset()
        };
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if result_scroll_off > 0 {
                    self.with_read_result_scroll(|s| {
                        s.scroll_by(-1);
                    });
                } else if n_bufs == 0 {
                    self.set_read_focus_active(ReadFocus::FunctionList);
                } else {
                    self.set_read_focus_active(ReadFocus::Args);
                    if self.contract_main_is_impl() {
                        self.arg_cursor_impl = self.arg_buffers_impl.len().saturating_sub(1);
                    } else {
                        self.arg_cursor = self.arg_buffers.len().saturating_sub(1);
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.with_read_result_scroll(|s| {
                    s.scroll_by(1);
                });
            }
            KeyCode::PageUp => {
                self.with_read_result_scroll(|s| {
                    s.page_up();
                });
            }
            KeyCode::PageDown => {
                self.with_read_result_scroll(|s| {
                    s.page_down();
                });
            }
            KeyCode::Home => {
                self.with_read_result_scroll(|s| {
                    s.home();
                });
            }
            KeyCode::End => {
                self.with_read_result_scroll(|s| {
                    s.end();
                });
            }
            KeyCode::Left => {
                self.set_read_focus_active(ReadFocus::Args);
                if n_bufs > 0 {
                    if self.contract_main_is_impl() {
                        self.arg_cursor_impl = self.arg_buffers_impl.len().saturating_sub(1);
                    } else {
                        self.arg_cursor = self.arg_buffers.len().saturating_sub(1);
                    }
                } else {
                    self.set_read_focus_active(ReadFocus::FunctionList);
                }
            }
            _ => {}
        }
        Command::None
    }

    fn handle_events_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Char('r') | KeyCode::Enter => {
                self.events = None;
                let _ = self.feed.events_tx.send(EventsRequest {
                    head_hint: self.events_head,
                    offset: self.events_offset,
                });
            }
            KeyCode::Char('n') => {
                let can_advance = matches!(
                    self.events.as_ref(),
                    Some(Ok(page)) if page.has_older
                );
                if can_advance {
                    self.events_offset = self.events_offset.saturating_add(1);
                    self.events = None;
                    let _ = self.feed.events_tx.send(EventsRequest {
                        head_hint: self.events_head,
                        offset: self.events_offset,
                    });
                }
            }
            KeyCode::Char('N') if self.events_offset > 0 => {
                self.events_offset -= 1;
                self.events = None;
                let _ = self.feed.events_tx.send(EventsRequest {
                    head_hint: self.events_head,
                    offset: self.events_offset,
                });
            }
            _ => return self.handle_paragraph_scroll_key(key),
        }
        Command::None
    }

    fn handle_storage_key(&mut self, key: KeyEvent) -> Command {
        match self.storage_focus {
            StorageFocus::Slot => match key.code {
                KeyCode::Up => {
                    self.focus_layer = DetailFocusLayer::Subtabs;
                }
                KeyCode::Down | KeyCode::Tab | KeyCode::Right => {
                    self.storage_focus = StorageFocus::Value;
                    self.storage_value_line = 0;
                    self.with_storage_value_scroll(|s| s.reset());
                }
                KeyCode::Backspace => {
                    self.slot_buffer.pop();
                }
                KeyCode::Char(c) if c.is_ascii_hexdigit() || c == 'x' || c == 'X' => {
                    self.slot_buffer.push(c);
                }
                KeyCode::Enter => match parse_slot(&self.slot_buffer) {
                    Ok(slot) => {
                        self.storage_slot_requested = Some(slot);
                        self.storage_result = None;
                        let _ = self.feed.storage_tx.send(StorageRequest { slot });
                        self.storage_focus = StorageFocus::Value;
                        self.storage_value_line = 0;
                        self.with_storage_value_scroll(|s| s.reset());
                    }
                    Err(msg) => {
                        self.storage_result = Some(Err(msg));
                    }
                },
                _ => {}
            },
            StorageFocus::Value => {
                let body = self.storage_value_body();
                let n = body.lines().count().max(1);
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if self.storage_value_line > 0 {
                            self.storage_value_line -= 1;
                        } else {
                            self.storage_focus = StorageFocus::Slot;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') if self.storage_value_line + 1 < n => {
                        self.storage_value_line += 1;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {}
                    KeyCode::PageUp => {
                        self.storage_value_line = self.storage_value_line.saturating_sub(10);
                    }
                    KeyCode::PageDown => {
                        self.storage_value_line = (self.storage_value_line + 10).min(n - 1);
                    }
                    KeyCode::Home => {
                        self.storage_value_line = 0;
                    }
                    KeyCode::End => {
                        self.storage_value_line = n - 1;
                    }
                    KeyCode::Left => {
                        self.storage_focus = StorageFocus::Slot;
                    }
                    _ => {}
                }
            }
        }
        Command::None
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers (main body)
// ---------------------------------------------------------------------------

impl AddressDetailScreen {
    fn render_overview(&self, frame: &mut Frame<'_>, area: Rect) {
        let body = overview_body(
            self.current.as_ref(),
            self.account_tx_page.as_ref(),
            self.transfers.as_ref(),
            self.holdings.as_ref(),
        );
        let content_lines = body.lines().count() as u16;
        let viewport = area.height.saturating_sub(2);
        let cap = content_lines.saturating_sub(viewport);
        self.scroll_cap.set(cap);
        let offset = self.scroll.min(cap);
        frame.render_widget(
            Paragraph::new(body)
                .wrap(Wrap { trim: false })
                .scroll((offset, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_outline())
                        .title("Overview"),
                ),
            area,
        );
    }

    fn render_transactions(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.body_outline())
            .title("Transactions");
        match self.account_tx_page.as_ref() {
            None => {
                frame.render_widget(Paragraph::new("Loading transactions...").block(block), area)
            }
            Some(page) if page.txs.is_empty() => frame.render_widget(
                Paragraph::new("No normal transactions found for this address.").block(block),
                area,
            ),
            Some(page) => {
                let items: Vec<ListItem> = page
                    .txs
                    .iter()
                    .map(|tx| ListItem::new(render_account_tx_row(tx)))
                    .collect();
                let mut state = self.tx_list_state;
                frame.render_stateful_widget(
                    List::new(items)
                        .block(block)
                        .highlight_style(
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .bg(Color::Indexed(238)),
                        )
                        .highlight_symbol("> "),
                    area,
                    &mut state,
                );
            }
        }
    }

    fn render_transfers(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.body_outline())
            .title("Transfers");
        match self.transfers.as_ref() {
            None => {
                frame.render_widget(Paragraph::new("Loading transfers...").block(block), area)
            }
            Some(page) if page.events.is_empty() => frame.render_widget(
                Paragraph::new("No asset transfers found for this address.").block(block),
                area,
            ),
            Some(page) => {
                let items: Vec<ListItem> = page
                    .events
                    .iter()
                    .map(|event| ListItem::new(render_transfer_row(event)))
                    .collect();
                let mut state = self.transfers_list_state;
                frame.render_stateful_widget(
                    List::new(items)
                        .block(block)
                        .highlight_style(
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .bg(Color::Indexed(238)),
                        )
                        .highlight_symbol("> "),
                    area,
                    &mut state,
                );
            }
        }
    }

    fn render_tokens(&self, frame: &mut Frame<'_>, area: Rect) {
        match self.holdings.as_ref() {
            None => frame.render_widget(
                Paragraph::new("Loading tokens...")
                    .block(Block::default().borders(Borders::ALL).title("Tokens")),
                area,
            ),
            Some(holdings) if holdings.is_empty() => frame.render_widget(
                Paragraph::new("No ERC-20 holdings found for this address.")
                    .block(Block::default().borders(Borders::ALL).title("Tokens")),
                area,
            ),
            Some(holdings) => {
                let summary = portfolio_summary(holdings);
                let chart_rows = u16::try_from(summary.top_by_usd.len().min(5)).unwrap_or(0);
                let chart_block_height = if chart_rows == 0 { 0 } else { chart_rows + 2 };
                let tokens_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Length(chart_block_height),
                        Constraint::Min(3),
                    ])
                    .split(area);

                frame.render_widget(
                    Paragraph::new(portfolio_header_line(&summary)).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Portfolio USD"),
                    ),
                    tokens_chunks[0],
                );

                if chart_block_height > 0 {
                    let bar_width = (tokens_chunks[1].width as usize).saturating_sub(30).max(5);
                    frame.render_widget(
                        Paragraph::new(render_top_distribution(&summary, bar_width))
                            .block(Block::default().borders(Borders::ALL).title("Top 5 by USD")),
                        tokens_chunks[1],
                    );
                }

                let items: Vec<ListItem> = holdings
                    .iter()
                    .map(|h| ListItem::new(render_token_row(h)))
                    .collect();
                let mut state = self.token_list_state;
                frame.render_stateful_widget(
                    List::new(items)
                        .block(Block::default().borders(Borders::ALL).title("Tokens"))
                        .highlight_style(
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .bg(Color::Indexed(238)),
                        )
                        .highlight_symbol("> "),
                    tokens_chunks[2],
                    &mut state,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Token sub-tab rendering
// ---------------------------------------------------------------------------

impl AddressDetailScreen {
    fn render_token_overview(&self, frame: &mut Frame<'_>, area: Rect) {
        let body = match self.token_probe {
            TokenProbeState::IsToken(ref ov) if ov.is_incomplete() => {
                render_incomplete_body(ov, self.address)
            }
            TokenProbeState::IsToken(ref ov) => {
                let price_cell = format_price_lookup(&self.token_price);
                let synth = TokenOverview {
                    metadata: ov.metadata.clone(),
                    total_supply: ov.total_supply,
                    price: self.token_price.clone(),
                };
                let mcap = synth
                    .market_cap()
                    .map(format_market_cap)
                    .unwrap_or_else(|| "-".to_string());
                format!(
                    "Address       {addr}\n\
Symbol        {symbol}\n\
Name          {name}\n\
Decimals      {decimals}\n\
Total supply  {supply} (raw)\n\
\n\
Price         {price}\n\
Market cap    {mcap}\n\
\n\
[Tab] cycle tabs    [1] 1d  [2] 1m  [3] 1y    [] / []] sub-tabs    [Esc] back",
                    addr = ov.metadata.address.to_hex(),
                    symbol = ov.metadata.symbol,
                    name = ov.metadata.name,
                    decimals = ov.metadata.decimals,
                    supply = ov.total_supply,
                    price = price_cell,
                    mcap = mcap,
                )
            }
            _ => "Loading...".to_string(),
        };
        let content_lines = body.lines().count() as u16;
        let viewport = area.height.saturating_sub(2);
        let cap = content_lines.saturating_sub(viewport);
        self.scroll_cap.set(cap);
        let offset = self.scroll.min(cap);
        frame.render_widget(
            Paragraph::new(body)
                .wrap(Wrap { trim: false })
                .scroll((offset, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_outline())
                        .title("Overview"),
                ),
            area,
        );
    }

    fn render_token_transfers(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.body_outline())
            .title("Transfers (ERC-20)");
        match self.token_transfers.as_ref() {
            None => frame.render_widget(Paragraph::new("Loading transfers...").block(block), area),
            Some(page) if page.events.is_empty() => frame.render_widget(
                Paragraph::new("No transfers found for this token yet.").block(block),
                area,
            ),
            Some(page) => {
                let items: Vec<ListItem> = page
                    .events
                    .iter()
                    .map(|e| ListItem::new(render_token_transfer_row(e)))
                    .collect();
                let mut state = self.token_transfers_list_state;
                frame.render_stateful_widget(
                    List::new(items)
                        .block(block)
                        .highlight_style(
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .bg(Color::Indexed(238)),
                        )
                        .highlight_symbol("> "),
                    area,
                    &mut state,
                );
            }
        }
    }

    fn render_token_chart(&self, frame: &mut Frame<'_>, area: Rect) {
        let title = format!("Price chart ({})", self.active_token_window.label());
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.body_outline())
            .title(title);
        let series = match self.token_series.get(&self.active_token_window) {
            None => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "Loading {label} price history...\n\
[1] 1d (1h)  [2] 1m (1d)  [3] 1y (1w)",
                        label = self.active_token_window.label(),
                    ))
                    .block(block),
                    area,
                );
                return;
            }
            Some(s) => s,
        };
        if series.points.is_empty() {
            frame.render_widget(
                Paragraph::new(format!(
                    "No price data for window {label}.\n\
[1] 1d (1h)  [2] 1m (1d)  [3] 1y (1w)",
                    label = self.active_token_window.label(),
                ))
                .block(block),
                area,
            );
            return;
        }
        let data: Vec<(f64, f64)> = series
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| (i as f64, p.value))
            .collect();
        let (lo, hi) = series.y_bounds().unwrap_or((0.0, 1.0));
        let y_min = if (hi - lo).abs() < f64::EPSILON {
            lo - 0.05 * lo.abs().max(1.0)
        } else {
            lo - (hi - lo) * 0.05
        };
        let y_max = if (hi - lo).abs() < f64::EPSILON {
            hi + 0.05 * hi.abs().max(1.0)
        } else {
            hi + (hi - lo) * 0.05
        };
        let x_max = (series.points.len().saturating_sub(1)) as f64;
        let datasets = vec![
            Dataset::default()
                .name("usd")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Cyan))
                .data(&data),
        ];
        let x_axis = Axis::default()
            .bounds([0.0, x_max.max(1.0)])
            .labels(vec![Span::raw("older"), Span::raw("now")])
            .style(Style::default().fg(Color::DarkGray));
        let y_axis = Axis::default()
            .bounds([y_min, y_max])
            .labels(vec![
                Span::raw(format_price(y_min)),
                Span::raw(format_price((y_min + y_max) / 2.0)),
                Span::raw(format_price(y_max)),
            ])
            .style(Style::default().fg(Color::DarkGray));
        let chart = Chart::new(datasets)
            .block(block)
            .x_axis(x_axis)
            .y_axis(y_axis);
        frame.render_widget(chart, area);
    }
}

// ---------------------------------------------------------------------------
// Contract sub-tab rendering
// ---------------------------------------------------------------------------

impl AddressDetailScreen {
    fn render_contract_overview(&self, frame: &mut Frame<'_>, area: Rect) {
        let body = contract_overview_body(
            self.address,
            self.contract_overview.as_ref(),
            self.source.as_ref(),
        );
        let offset = self.bound_scroll_for(&body, area);
        frame.render_widget(
            Paragraph::new(body)
                .wrap(Wrap { trim: false })
                .scroll((offset, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_outline())
                        .title("Overview"),
                ),
            area,
        );
    }

    /// Contract **implementation** dossier (proxy addresses only).
    fn render_contract_overview_impl(&self, frame: &mut Frame<'_>, area: Rect) {
        let impl_addr = self
            .contract_overview_impl
            .as_ref()
            .map(|o| o.account.address)
            .or_else(|| {
                self.contract_overview
                    .as_ref()
                    .and_then(|c| c.proxy)
                    .map(|p| p.implementation)
            })
            .unwrap_or(self.address);
        let body = contract_overview_body(
            impl_addr,
            self.contract_overview_impl.as_ref(),
            self.source_impl.as_ref(),
        );
        let offset = self.bound_scroll_for(&body, area);
        frame.render_widget(
            Paragraph::new(body)
                .wrap(Wrap { trim: false })
                .scroll((offset, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_outline())
                        .title("Overview (impl)"),
                ),
            area,
        );
    }

    fn render_abi_tab(&self, frame: &mut Frame<'_>, area: Rect) {
        let src = if self.contract_main_is_impl() {
            self.source_impl.as_ref()
        } else {
            self.source.as_ref()
        };
        let (title, body) = abi_body(src);
        let offset = self.bound_scroll_for(&body, area);
        frame.render_widget(
            Paragraph::new(body)
                .wrap(Wrap { trim: false })
                .scroll((offset, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_outline())
                        .title(title),
                ),
            area,
        );
    }

    fn render_source_tab(&self, frame: &mut Frame<'_>, area: Rect) {
        let source = if self.contract_main_is_impl() {
            self.source_impl.as_ref()
        } else {
            self.source.as_ref()
        };
        let Some(source) = source else {
            frame.render_widget(
                Paragraph::new("Loading source...").block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_outline())
                        .title("Source"),
                ),
                area,
            );
            return;
        };
        if !source.is_verified || source.files.is_empty() {
            frame.render_widget(
                Paragraph::new(
                    "Contract is not verified on Etherscan.\n\
Open the ABI tab for a raw ABI read (empty when unverified) or come back\n\
once a decompiler integration lands (see plan/7 section 13).",
                )
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_outline())
                        .title("Source"),
                ),
                area,
            );
            return;
        }
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);
        let files_border =
            self.split_pane_border(matches!(self.source_focus_active(), SourceFocus::Files));
        let viewer_border =
            self.split_pane_border(matches!(self.source_focus_active(), SourceFocus::Viewer));
        let items: Vec<ListItem> = source
            .files
            .iter()
            .map(|f| ListItem::new(f.path.clone()))
            .collect();
        let mut state = if self.contract_main_is_impl() {
            self.file_list_state_impl
        } else {
            self.file_list_state
        };
        frame.render_stateful_widget(
            List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(files_border)
                        .title("Files"),
                )
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::Indexed(238)),
                )
                .highlight_symbol("> "),
            columns[0],
            &mut state,
        );
        let file = self.selected_file();
        let title = file
            .map(|f| f.path.clone())
            .unwrap_or_else(|| "Source".to_string());
        let content = file.map(|f| f.content.as_str()).unwrap_or("");
        let offset = self.bound_scroll_for(content, columns[1]);
        let is_solidity = file
            .map(|f| f.path.to_ascii_lowercase().ends_with(".sol"))
            .unwrap_or(false);
        let paragraph = if is_solidity && !content.is_empty() {
            Paragraph::new(highlight_solidity(content))
        } else {
            Paragraph::new(content.to_string())
        };
        frame.render_widget(
            paragraph
                .wrap(Wrap { trim: false })
                .scroll((offset, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(viewer_border)
                        .title(title),
                ),
            columns[1],
        );
    }

    fn render_read_tab(&self, frame: &mut Frame<'_>, area: Rect) {
        let abi_loading = if self.contract_main_is_impl() {
            self.source_impl.is_none()
        } else {
            self.source.is_none()
        };
        if abi_loading {
            frame.render_widget(
                Paragraph::new("Loading ABI...").block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_outline())
                        .title("Read"),
                ),
                area,
            );
            return;
        }
        let fns = if self.contract_main_is_impl() {
            &self.functions_impl
        } else {
            &self.functions
        };
        if fns.is_empty() {
            frame.render_widget(
                Paragraph::new(
                    "No callable functions in this ABI.\n\
Contract may be unverified or expose only events / constructors.",
                )
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_outline())
                        .title("Read"),
                ),
                area,
            );
            return;
        }
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);
        let rf = self.read_focus_active();
        let list_border =
            self.split_pane_border(matches!(rf, ReadFocus::FunctionList));
        let sig_border = self.split_pane_border(false);
        let args_border = self.split_pane_border(matches!(rf, ReadFocus::Args));
        let result_border = self.split_pane_border(matches!(rf, ReadFocus::Result));
        let items: Vec<ListItem> = fns
            .iter()
            .map(|f| {
                let marker = if f.is_read_only { " " } else { "!" };
                ListItem::new(format!("{marker} {}", f.signature()))
            })
            .collect();
        let mut state = if self.contract_main_is_impl() {
            self.function_list_state_impl
        } else {
            self.function_list_state
        };
        let list_title = match rf {
            ReadFocus::FunctionList => "Functions (focused)",
            ReadFocus::Args | ReadFocus::Result => "Functions",
        };
        frame.render_stateful_widget(
            List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(list_border)
                        .title(list_title),
                )
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::Indexed(238)),
                )
                .highlight_symbol("> "),
            columns[0],
            &mut state,
        );
        let detail_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(5),
                Constraint::Min(5),
            ])
            .split(columns[1]);
        let function = self.selected_function();
        let meta_text = match function {
            Some(f) => {
                let mutability = if f.is_read_only {
                    "view/pure"
                } else {
                    "!! state-changing (not executable)"
                };
                let outputs = if f.outputs.is_empty() {
                    "()".to_string()
                } else {
                    f.outputs
                        .iter()
                        .map(|o| o.kind.canonical())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                format!("{sig}\n-> ({outputs})\n{mutability}", sig = f.signature(),)
            }
            None => "(no function selected)".to_string(),
        };
        frame.render_widget(
            Paragraph::new(meta_text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(sig_border)
                    .title("Signature"),
            ),
            detail_chunks[0],
        );
        let args_title = match rf {
            ReadFocus::Args => "Arguments (focused, [Enter] to execute)",
            ReadFocus::Result => "Arguments",
            ReadFocus::FunctionList => "Arguments ([Tab*] to focus)",
        };
        let arg_buffers = if self.contract_main_is_impl() {
            &self.arg_buffers_impl
        } else {
            &self.arg_buffers
        };
        let arg_cursor = if self.contract_main_is_impl() {
            self.arg_cursor_impl
        } else {
            self.arg_cursor
        };
        let args_body = match function {
            Some(f) if f.inputs.is_empty() => "(no arguments)".to_string(),
            Some(f) => {
                let mut lines = Vec::with_capacity(f.inputs.len());
                for (idx, (param, buf)) in f.inputs.iter().zip(arg_buffers.iter()).enumerate() {
                    let cursor = if matches!(rf, ReadFocus::Args) && idx == arg_cursor {
                        ">"
                    } else {
                        " "
                    };
                    lines.push(format!(
                        "{cursor} {name} ({ty}) = {buf}",
                        name = if param.name.is_empty() {
                            format!("arg{idx}")
                        } else {
                            param.name.clone()
                        },
                        ty = param.kind.canonical(),
                        buf = buf,
                    ));
                }
                lines.join("\n")
            }
            None => String::new(),
        };
        frame.render_widget(
            Paragraph::new(args_body).wrap(Wrap { trim: false }).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(args_border)
                    .title(args_title),
            ),
            detail_chunks[1],
        );
        let (last_res, last_for) = if self.contract_main_is_impl() {
            (&self.last_result_impl, &self.last_result_for_impl)
        } else {
            (&self.last_result, &self.last_result_for)
        };
        let result_body = match (
            last_res.as_ref(),
            last_for.as_deref(),
            function.map(|f| f.signature()),
        ) {
            (Some(Ok(values)), Some(sig), Some(current)) if sig == current => {
                if values.is_empty() {
                    "(no return values)".to_string()
                } else {
                    values
                        .iter()
                        .enumerate()
                        .map(|(i, v)| format!("[{i}] {v}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            }
            (Some(Err(msg)), Some(sig), Some(current)) if sig == current => {
                format!("ERROR: {msg}")
            }
            _ => "(press Enter on the arguments pane to execute)".to_string(),
        };
        let res_offset = self.bound_read_result_for(&result_body, detail_chunks[2]);
        frame.render_widget(
            Paragraph::new(result_body)
                .wrap(Wrap { trim: false })
                .scroll((res_offset, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(result_border)
                        .title("Result"),
                ),
            detail_chunks[2],
        );
    }

    fn render_events_tab(&self, frame: &mut Frame<'_>, area: Rect) {
        let page_num = self.events_offset + 1;
        let header = match self.events.as_ref() {
            Some(Ok(page)) => format!(
                "Window #{from}..#{to}  (page {page_num})   [n] older  [N] newer  [r] refresh",
                from = page.window_from.value(),
                to = page.window_to.value(),
            ),
            _ => format!("Window (pending)  (page {page_num})   [n] older  [N] newer  [r] refresh"),
        };
        let body = match self.events.as_ref() {
            None => format!("{header}\n\nLoading events..."),
            Some(Err(msg)) => format!("{header}\n\nERROR: {msg}"),
            Some(Ok(page)) if page.logs.is_empty() => {
                let tail = if page.has_older {
                    "\n[n] page to older blocks"
                } else {
                    ""
                };
                format!("{header}\n\nNo events in this window.{tail}")
            }
            Some(Ok(page)) => {
                let mut out = format!("{header}\n\n");
                for (idx, log) in page.logs.iter().enumerate() {
                    let topic0 = log
                        .topics
                        .first()
                        .map(|t| format!("0x{}", hex::encode(t)))
                        .unwrap_or_else(|| "(anonymous)".to_string());
                    out.push_str(&format!("#{idx}  {topic0}\n"));
                    for (ti, topic) in log.topics.iter().enumerate().skip(1) {
                        out.push_str(&format!("  t{ti}:   0x{}\n", hex::encode(topic)));
                    }
                    if !log.data.is_empty() {
                        out.push_str(&format!("  data: 0x{}\n", hex::encode(&log.data)));
                    }
                    out.push('\n');
                }
                out
            }
        };
        let offset = self.bound_scroll_for(&body, area);
        frame.render_widget(
            Paragraph::new(body)
                .wrap(Wrap { trim: false })
                .scroll((offset, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.body_outline())
                        .title("Events"),
                ),
            area,
        );
    }

    fn render_storage_tab(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(3)])
            .split(area);
        let prompt = format!(
            "Slot (decimal or 0x-hex): {}\n[Enter] to read, [Backspace] to edit",
            self.slot_buffer,
        );
        let slot_border = self.split_pane_border(matches!(self.storage_focus, StorageFocus::Slot));
        let value_border =
            self.split_pane_border(matches!(self.storage_focus, StorageFocus::Value));
        frame.render_widget(
            Paragraph::new(prompt).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(slot_border)
                    .title("Slot"),
            ),
            chunks[0],
        );
        let body = self.storage_value_body();
        let v_offset = self.bound_storage_value_for(&body, chunks[1]);
        let line_strings: Vec<&str> = body.lines().collect();
        let n = line_strings.len().max(1);
        let sel = self.storage_value_line.min(n.saturating_sub(1));
        let highlight_value = matches!(self.storage_focus, StorageFocus::Value);
        let styled: Vec<Line<'static>> = if line_strings.is_empty() {
            vec![Line::from(body.clone())]
        } else {
            line_strings
                .into_iter()
                .enumerate()
                .map(|(i, line)| {
                    let selected = highlight_value && i == sel;
                    let style = if selected {
                        Style::default()
                            .bg(Color::Indexed(238))
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    Line::from(Span::styled(line.to_string(), style))
                })
                .collect()
        };
        frame.render_widget(
            Paragraph::new(styled)
                .wrap(Wrap { trim: false })
                .scroll((v_offset, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(value_border)
                        .title("Value"),
                ),
            chunks[1],
        );
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

fn clamp_selection(state: &mut ListState, len: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let current = state.selected().unwrap_or(0);
    state.select(Some(current.min(len - 1)));
}

fn overview_body(
    overview: Option<&AddressOverview>,
    account_tx_page: Option<&AccountTxPage>,
    transfers: Option<&TransferPage>,
    holdings: Option<&Vec<TokenHolding>>,
) -> String {
    match overview {
        None => "Loading...".to_string(),
        Some(ov) => {
            let kind = match ov.kind {
                AddressKind::Eoa {
                    delegated_to: Some(_),
                } => "EOA (7702 delegated)",
                AddressKind::Eoa { delegated_to: None } => "EOA",
                AddressKind::Contract => "Contract",
            };
            let tx_count = account_tx_page
                .map(|p| p.txs.len())
                .map(|n| n.to_string())
                .unwrap_or_else(|| "loading...".to_string());
            let xfer_count = transfers
                .map(|p| p.events.len())
                .map(|n| n.to_string())
                .unwrap_or_else(|| "loading...".to_string());
            let token_count = holdings
                .map(|h| h.len())
                .map(|n| n.to_string())
                .unwrap_or_else(|| "loading...".to_string());
            format!(
                "Address       {addr}\n\
Kind          {kind}\n\
Balance       {balance} wei\n\
Nonce         {nonce}\n\
\n\
Txs loaded       {tx_count}\n\
Transfers loaded {xfer_count}\n\
Tokens loaded    {token_count}\n\
\n\
[Tab] cycle tabs    [Enter] open selection    [Esc] back",
                addr = ov.address.to_hex(),
                kind = kind,
                balance = ov.balance.value(),
                nonce = ov.nonce,
                tx_count = tx_count,
                xfer_count = xfer_count,
                token_count = token_count,
            )
        }
    }
}

fn render_account_tx_row(tx: &AccountTx) -> String {
    let from = short_addr(tx.from.to_hex().as_str());
    let to = tx
        .to
        .map(|a| short_addr(a.to_hex().as_str()))
        .unwrap_or_else(|| "(create)".to_string());
    let h = tx.tx_hash.to_hex();
    let short_h = if h.len() > 14 {
        format!("{}…{}", &h[..8], &h[h.len() - 6..])
    } else {
        h
    };
    format!(
        "#{block:<10}  {short_h}  {from} -> {to}  {value} wei",
        block = tx.block_number.value(),
        short_h = short_h,
        from = from,
        to = to,
        value = tx.value.value(),
    )
}

fn render_token_row(h: &TokenHolding) -> String {
    let price_column = match &h.price {
        PriceLookup::Available(p) => {
            let usd = token_usd_value(h.balance.value(), h.metadata.decimals, p.value);
            format!("{}   @{}", format_usd(usd), format_usd(p.value))
        }
        PriceLookup::Unsupported { provider } => format!("(not indexed by {provider})"),
        PriceLookup::Pending => "(price loading…)".to_string(),
    };
    format!(
        "{symbol:<10}  {balance:<20}  d={decimals:<2}  {price_column:<36}  {contract}",
        symbol = h.metadata.symbol,
        balance = h.balance.value(),
        decimals = h.metadata.decimals,
        price_column = price_column,
        contract = h.metadata.address.to_hex(),
    )
}

fn render_transfer_row(event: &TransferEvent) -> String {
    let cat = event.category.label();
    let from = short_addr(event.from.to_hex().as_str());
    let to = event
        .to
        .map(|a| short_addr(a.to_hex().as_str()))
        .unwrap_or_else(|| "(create)".to_string());
    let amount = format_amount(event);
    let block = event.block_number.value();
    format!(
        "[{cat:>7}] #{block:<10}  {from} -> {to}  {amount}",
        cat = cat,
        block = block,
        from = from,
        to = to,
        amount = amount,
    )
}

fn render_token_transfer_row(event: &TransferEvent) -> String {
    let from = short_addr(event.from.to_hex().as_str());
    let to = event
        .to
        .map(|a| short_addr(a.to_hex().as_str()))
        .unwrap_or_else(|| "(create)".to_string());
    let amount = format_token_transfer_amount(event);
    format!(
        "#{block:<10}  {from} -> {to}  {amount}",
        block = event.block_number.value(),
        from = from,
        to = to,
        amount = amount,
    )
}

fn format_amount(event: &TransferEvent) -> String {
    let symbol = event.asset.symbol();
    match &event.asset {
        TransferAsset::Native { .. } => {
            format!("{value} wei {symbol}", value = event.value.value())
        }
        TransferAsset::Erc20 { decimals, .. } => {
            format!(
                "{v} {symbol}  (raw 0x{raw:x}, d={decimals})",
                v = event.value.value(),
                raw = event.value.value(),
                decimals = decimals,
            )
        }
        TransferAsset::Nft { token_id, kind, .. } => {
            let kind_label = match kind {
                NftKind::Erc721 => "721",
                NftKind::Erc1155 => "1155",
            };
            format!("{symbol} #{token_id} ({kind_label})")
        }
    }
}

fn format_token_transfer_amount(event: &TransferEvent) -> String {
    match &event.asset {
        TransferAsset::Erc20 {
            symbol, decimals, ..
        } => {
            let human = raw_to_human(event.value.value(), *decimals);
            format!("{human} {symbol}")
        }
        TransferAsset::Native { symbol } => {
            format!("{} {symbol}", event.value.value())
        }
        TransferAsset::Nft {
            symbol, token_id, ..
        } => format!("{symbol} #{token_id}"),
    }
}

fn short_addr(s: &str) -> String {
    if s.len() <= 12 {
        return s.to_string();
    }
    format!("{}...{}", &s[..6], &s[s.len() - 4..])
}

fn raw_to_human(raw: u128, decimals: u8) -> String {
    if decimals == 0 {
        return raw.to_string();
    }
    let divisor = 10u128.pow(u32::from(decimals));
    let whole = raw / divisor;
    let frac = raw % divisor;
    let frac_str = format!("{:0width$}", frac, width = decimals as usize);
    let frac_trim = frac_str.trim_end_matches('0');
    if frac_trim.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{frac_trim}")
    }
}

// ---------------------------------------------------------------------------
// CSV export helpers
// ---------------------------------------------------------------------------

fn csv_for_account_tx(page: Option<&AccountTxPage>) -> String {
    let mut buf = String::from("block,tx_hash,from,to,value_wei\n");
    let Some(page) = page else {
        return buf;
    };
    for tx in &page.txs {
        let to = tx
            .to
            .map(|a| a.to_hex())
            .unwrap_or_else(|| "".to_string());
        buf.push_str(&format!(
            "{block},{tx},{from},{to},{value}\n",
            block = tx.block_number.value(),
            tx = tx.tx_hash.to_hex(),
            from = tx.from.to_hex(),
            to = to,
            value = tx.value.value(),
        ));
    }
    buf
}

fn csv_for_overview(overview: Option<&AddressOverview>) -> String {
    let mut buf = String::from("address,ens,kind,balance,nonce\n");
    if let Some(ov) = overview {
        let kind = match ov.kind {
            AddressKind::Eoa {
                delegated_to: Some(_),
            } => "eoa_delegated",
            AddressKind::Eoa { delegated_to: None } => "eoa",
            AddressKind::Contract => "contract",
        };
        let ens = ov.ens_name.as_deref().unwrap_or("");
        buf.push_str(&format!(
            "{addr},{ens},{kind},{bal},{nonce}\n",
            addr = ov.address.to_hex(),
            ens = ens,
            kind = kind,
            bal = ov.balance.value(),
            nonce = ov.nonce,
        ));
    }
    buf
}

fn csv_for_transfers(page: Option<&TransferPage>) -> String {
    let mut buf = String::from("block,tx_hash,category,from,to,asset,symbol,decimals,value\n");
    let Some(page) = page else { return buf };
    for event in &page.events {
        let (asset_kind, symbol, decimals) = match &event.asset {
            TransferAsset::Native { symbol } => ("native", symbol.as_str(), String::new()),
            TransferAsset::Erc20 {
                symbol, decimals, ..
            } => ("erc20", symbol.as_str(), decimals.to_string()),
            TransferAsset::Nft { kind, .. } => match kind {
                NftKind::Erc721 => ("erc721", "", String::new()),
                NftKind::Erc1155 => ("erc1155", "", String::new()),
            },
        };
        let to = event
            .to
            .map(|a| a.to_hex())
            .unwrap_or_else(|| "0x".to_string());
        buf.push_str(&format!(
            "{block},{tx},{cat},{from},{to},{asset_kind},{symbol},{decimals},{value}\n",
            block = event.block_number.value(),
            tx = event.tx_hash.to_hex(),
            cat = event.category.label().to_ascii_lowercase(),
            from = event.from.to_hex(),
            to = to,
            asset_kind = asset_kind,
            symbol = symbol,
            decimals = decimals,
            value = event.value.value(),
        ));
    }
    buf
}

fn csv_for_holdings(holdings: Option<&Vec<TokenHolding>>) -> String {
    let mut buf =
        String::from("symbol,name,contract,decimals,balance,price_usd,price_status,value_usd\n");
    let Some(holdings) = holdings else { return buf };
    for h in holdings {
        let (price_column, status, value_column) = match &h.price {
            PriceLookup::Available(p) => {
                let usd_value = token_usd_value(h.balance.value(), h.metadata.decimals, p.value);
                (
                    format_csv_number(p.value),
                    "available".to_string(),
                    format_csv_number(usd_value),
                )
            }
            PriceLookup::Unsupported { provider } => (
                String::new(),
                format!("unsupported:{provider}"),
                String::new(),
            ),
            PriceLookup::Pending => (String::new(), "pending".to_string(), String::new()),
        };
        buf.push_str(&format!(
            "{sym},{name},{contract},{dec},{bal},{price_column},{status},{value_column}\n",
            sym = h.metadata.symbol,
            name = h.metadata.name,
            contract = h.metadata.address.to_hex(),
            dec = h.metadata.decimals,
            bal = h.balance.value(),
            price_column = price_column,
            status = status,
            value_column = value_column,
        ));
    }
    buf
}

pub fn token_usd_value(raw_balance: u128, decimals: u8, price_usd: f64) -> f64 {
    if decimals == 0 {
        return (raw_balance as f64) * price_usd;
    }
    let power = u32::from(decimals.min(38));
    let divisor = 10f64.powi(power as i32);
    (raw_balance as f64) / divisor * price_usd
}

// ---------------------------------------------------------------------------
// Portfolio summary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct PortfolioTopEntry {
    pub symbol: String,
    pub usd_value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortfolioSummary {
    pub total_usd: f64,
    pub priced: usize,
    pub not_priced: usize,
    pub top_by_usd: Vec<PortfolioTopEntry>,
}

#[must_use]
pub fn portfolio_summary(holdings: &[TokenHolding]) -> PortfolioSummary {
    let mut total = 0.0f64;
    let mut priced = 0usize;
    let mut not_priced = 0usize;
    let mut ranked: Vec<PortfolioTopEntry> = Vec::new();
    for h in holdings {
        match &h.price {
            PriceLookup::Available(p) => {
                let usd = token_usd_value(h.balance.value(), h.metadata.decimals, p.value);
                if usd.is_finite() {
                    total += usd;
                }
                priced += 1;
                ranked.push(PortfolioTopEntry {
                    symbol: h.metadata.symbol.clone(),
                    usd_value: usd,
                });
            }
            PriceLookup::Unsupported { .. } | PriceLookup::Pending => {
                not_priced += 1;
            }
        }
    }
    ranked.sort_by(|a, b| b.usd_value.total_cmp(&a.usd_value));
    ranked.truncate(5);
    PortfolioSummary {
        total_usd: total,
        priced,
        not_priced,
        top_by_usd: ranked,
    }
}

fn portfolio_header_line(summary: &PortfolioSummary) -> String {
    let total = format_usd(summary.total_usd);
    if summary.not_priced == 0 {
        format!("Σ USD {total}  ({priced} priced)", priced = summary.priced)
    } else {
        format!(
            "Σ USD {total}  ({priced} priced, {not} not priced)",
            priced = summary.priced,
            not = summary.not_priced,
        )
    }
}

fn format_usd(v: f64) -> String {
    if !v.is_finite() {
        return "$?".to_string();
    }
    format!("${v:.2}")
}

pub fn render_top_distribution(summary: &PortfolioSummary, width: usize) -> String {
    if summary.top_by_usd.is_empty() {
        return "(no priced holdings yet)".to_string();
    }
    let max_symbol_len = summary
        .top_by_usd
        .iter()
        .map(|e| e.symbol.chars().count())
        .max()
        .unwrap_or(0)
        .max(3);
    let max_value = summary
        .top_by_usd
        .iter()
        .map(|e| e.usd_value)
        .fold(0.0f64, f64::max);
    let mut out = String::new();
    for entry in &summary.top_by_usd {
        let share = if max_value > 0.0 {
            (entry.usd_value / max_value).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let bar_len = ((share * width as f64).round() as usize).min(width);
        let bar: String = std::iter::repeat_n('█', bar_len).collect();
        let pad: String = std::iter::repeat_n(' ', width - bar_len).collect();
        out.push_str(&format!(
            "{symbol:<sym_w$}  {bar}{pad}  {value}\n",
            symbol = entry.symbol,
            bar = bar,
            pad = pad,
            value = format_usd(entry.usd_value),
            sym_w = max_symbol_len,
        ));
    }
    out.trim_end_matches('\n').to_string()
}

fn format_csv_number(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    let formatted = format!("{v:.6}");
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

// ---------------------------------------------------------------------------
// Contract overview body + ABI pretty-print
// ---------------------------------------------------------------------------

fn contract_overview_body(
    address: Address,
    overview: Option<&ContractOverview>,
    source: Option<&ContractSource>,
) -> String {
    let Some(ov) = overview else {
        return "Loading...".to_string();
    };
    let proxy_line = match ov.proxy {
        Some(info) => {
            let role = match info.kind {
                crate::domain::ProxyKind::Transparent => "admin",
                _ => "impl",
            };
            format!(
                "Proxy       {kind} [{source}] -> {role} {impl_addr}",
                kind = info.kind.label(),
                source = info.source.label(),
                role = role,
                impl_addr = info.implementation.to_hex(),
            )
        }
        None => "Proxy       not detected".to_string(),
    };
    let source_line = match source {
        Some(s) if s.is_verified => {
            let optimizer = if s.optimizer_enabled {
                format!("enabled ({} runs)", s.optimizer_runs)
            } else {
                "disabled".to_string()
            };
            format!(
                "Verified    yes\n\
Name        {name}\n\
Compiler    {compiler}\n\
Optimizer   {optimizer}\n\
License     {license}",
                name = if s.contract_name.is_empty() {
                    "(unknown)".to_string()
                } else {
                    s.contract_name.clone()
                },
                compiler = if s.compiler_version.is_empty() {
                    "(unknown)".to_string()
                } else {
                    s.compiler_version.clone()
                },
                optimizer = optimizer,
                license = if s.license.is_empty() {
                    "(unknown)".to_string()
                } else {
                    s.license.clone()
                },
            )
        }
        Some(_) => "Verified    no".to_string(),
        None => "Verified    (loading...)".to_string(),
    };
    format!(
        "Address     {addr}\n\
Balance     {balance} wei\n\
Nonce       {nonce}\n\
{proxy_line}\n\
\n\
{source_line}\n\
\n\
[ / ] sub-tabs    Tab — main tabs    Source: arrows pick file",
        addr = address.to_hex(),
        balance = ov.account.balance.value(),
        nonce = ov.account.nonce,
    )
}

fn abi_body(source: Option<&ContractSource>) -> (String, String) {
    let Some(source) = source else {
        return ("ABI".to_string(), "Loading ABI...".to_string());
    };
    if !source.is_verified || source.abi.trim().is_empty() {
        return (
            "ABI".to_string(),
            "No ABI available (contract unverified).".to_string(),
        );
    }
    let pretty = serde_json::from_str::<serde_json::Value>(&source.abi)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| source.abi.clone());
    ("ABI".to_string(), pretty)
}

fn parse_slot(raw: &str) -> Result<[u8; 32], String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("slot input is empty".into());
    }
    let (radix, digits) = if let Some(rest) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        (16, rest)
    } else {
        (10, trimmed)
    };
    if radix == 16 {
        if digits.len() > 64 {
            return Err(format!("slot hex too long ({} > 64)", digits.len()));
        }
        let padded = format!("{:0>64}", digits);
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(&padded, &mut bytes).map_err(|e| format!("invalid hex: {e}"))?;
        Ok(bytes)
    } else {
        let n: u128 = digits
            .parse()
            .map_err(|e| format!("invalid decimal: {e}"))?;
        let mut bytes = [0u8; 32];
        bytes[16..].copy_from_slice(&n.to_be_bytes());
        Ok(bytes)
    }
}

fn format_storage_word(slot: [u8; 32], word: &[u8; 32]) -> String {
    let hex_out = format!("0x{}", hex::encode(word));
    let slot_hex = format!("0x{}", hex::encode(slot));
    let as_u128 = if word[..16].iter().all(|b| *b == 0) {
        let mut buf = [0u8; 16];
        buf.copy_from_slice(&word[16..]);
        Some(u128::from_be_bytes(buf))
    } else {
        None
    };
    let as_address = if word[..12].iter().all(|b| *b == 0) {
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&word[12..]);
        Some(Address::from_bytes(bytes).to_hex())
    } else {
        None
    };
    let mut out = format!("Slot      {slot_hex}\nHex       {hex_out}\n");
    if let Some(n) = as_u128 {
        out.push_str(&format!("Decimal   {n}\n"));
    }
    if let Some(addr) = as_address {
        out.push_str(&format!("Address   {addr}\n"));
    }
    out
}

fn domain_error_message(err: &DomainError) -> String {
    match err {
        DomainError::ExecutionReverted { reason } => format!("revert: {reason}"),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Incomplete token copy (plan/8 §13.2)
// ---------------------------------------------------------------------------

fn render_incomplete_body(ov: &TokenOverview, address: Address) -> String {
    let addr_hex = ov.metadata.address.to_hex();
    let shown_addr = if addr_hex.is_empty() {
        address.to_hex()
    } else {
        addr_hex
    };
    let symbol_cell = if ov.metadata.symbol.is_empty() {
        "(none)".to_string()
    } else {
        ov.metadata.symbol.clone()
    };
    let name_cell = if ov.metadata.name.is_empty() {
        "(none)".to_string()
    } else {
        ov.metadata.name.clone()
    };
    format!(
        "Address       {addr}\n\
Symbol        {symbol}\n\
Name          {name}\n\
Decimals      {decimals}\n\
\n\
This address does not look like a standard ERC-20.\n\
Decimals / symbol are missing from alchemy_getTokenMetadata.\n\
\n\
[c] View as Contract    [Esc] back",
        addr = shown_addr,
        symbol = symbol_cell,
        name = name_cell,
        decimals = ov.metadata.decimals,
    )
}

fn format_price_lookup(lookup: &PriceLookup) -> String {
    match lookup {
        PriceLookup::Available(p) => format_price(p.value),
        PriceLookup::Unsupported { provider } => format!("(not indexed by {provider})"),
        PriceLookup::Pending => "loading...".to_string(),
    }
}

fn format_price(value: f64) -> String {
    if !value.is_finite() {
        return "-".to_string();
    }
    if value == 0.0 {
        return "$0".to_string();
    }
    let abs = value.abs();
    if abs >= 1.0 {
        format!("${value:.4}")
    } else if abs >= 0.01 {
        format!("${value:.6}")
    } else {
        format!("${value:.8}")
    }
}

fn format_market_cap(value: f64) -> String {
    if !value.is_finite() || value <= 0.0 {
        return "-".to_string();
    }
    let rounded = value.round() as u128;
    let with_commas = group_thousands(rounded);
    format!("${with_commas}")
}

fn group_thousands(mut n: u128) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut parts = Vec::new();
    while n > 0 {
        parts.push(format!("{:03}", n % 1000));
        n /= 1000;
    }
    let first = parts.pop().unwrap();
    let first = first.trim_start_matches('0');
    let first = if first.is_empty() { "0" } else { first };
    let mut out = String::from(first);
    for chunk in parts.into_iter().rev() {
        out.push(',');
        out.push_str(&chunk);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_thousands_formats_values_with_us_grouping() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(7), "7");
        assert_eq!(group_thousands(1234), "1,234");
        assert_eq!(group_thousands(1_000_000), "1,000,000");
    }

    #[test]
    fn raw_to_human_trims_trailing_zeros() {
        assert_eq!(raw_to_human(1_000_000, 6), "1");
        assert_eq!(raw_to_human(1_234_500, 6), "1.2345");
        assert_eq!(raw_to_human(0, 18), "0");
    }
}
