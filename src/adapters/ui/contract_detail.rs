//! Contract Detail screen.
//!
//! Tabs: Overview, Source, ABI. Read / Events / Storage land in the
//! follow-up commits of the plan-7 expansion. See
//! `plan/7-contract-detail.md` sections 12.3 (MVP) and 12.4.1.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::{
        highlight::highlight_solidity,
        screen::{Command, Screen},
        scroll::ScrollState,
    },
    domain::{
        AbiFunction, AbiParamType, AbiValue, Address, BlockNumber, Chain, ContractOverview,
        ContractSource, DecodedValue, DomainError, EventsPage, SourceFile, parse_abi_functions,
    },
};

/// Single-request envelope for the Read tab: the screen sends a
/// `ReadRequest`, the background task sends the response back on
/// `read_rx`.
#[derive(Debug, Clone)]
pub struct ReadRequest {
    pub function: AbiFunction,
    pub args: Vec<AbiValue>,
}

/// Result of a single read invocation. `Err` carries the user-facing
/// reason so the UI can render it on the result pane.
pub type ReadResult = Result<Vec<DecodedValue>, DomainError>;

/// Request shape for the Events tab. `head_hint` is `Some` after
/// the first page has been loaded so subsequent pages do not
/// re-resolve the chain head; `offset = 0` is the newest window.
/// See `plan/7-contract-detail.md` §12.5.3.
#[derive(Debug, Clone, Copy)]
pub struct EventsRequest {
    pub head_hint: Option<BlockNumber>,
    pub offset: u32,
}

/// Result of a single Events tab refresh.
pub type EventsResult = Result<EventsPage, DomainError>;

/// Request shape for the Storage tab. Slot is already a 32-byte
/// buffer (parsed from the user-supplied decimal or hex string).
#[derive(Debug, Clone)]
pub struct StorageRequest {
    pub slot: [u8; 32],
}

/// Result of a single Storage tab read.
pub type StorageResult = Result<[u8; 32], DomainError>;

pub struct ContractFeed {
    pub input_tx: UnboundedSender<Address>,
    pub updates_rx: UnboundedReceiver<ContractOverview>,
    pub source_rx: UnboundedReceiver<ContractSource>,
    pub read_tx: UnboundedSender<ReadRequest>,
    pub read_rx: UnboundedReceiver<ReadResult>,
    pub events_tx: UnboundedSender<EventsRequest>,
    pub events_rx: UnboundedReceiver<EventsResult>,
    pub storage_tx: UnboundedSender<StorageRequest>,
    pub storage_rx: UnboundedReceiver<StorageResult>,
}

pub struct ContractFeedSender {
    pub updates_tx: UnboundedSender<ContractOverview>,
    pub source_tx: UnboundedSender<ContractSource>,
    pub read_rx: UnboundedReceiver<ReadRequest>,
    pub read_tx: UnboundedSender<ReadResult>,
    pub events_rx: UnboundedReceiver<EventsRequest>,
    pub events_tx: UnboundedSender<EventsResult>,
    pub storage_rx: UnboundedReceiver<StorageRequest>,
    pub storage_tx: UnboundedSender<StorageResult>,
    pub input_rx: UnboundedReceiver<Address>,
}

#[must_use]
pub fn contract_feed() -> (ContractFeed, ContractFeedSender) {
    let (input_tx, input_rx) = unbounded_channel();
    let (updates_tx, updates_rx) = unbounded_channel();
    let (source_tx, source_rx) = unbounded_channel();
    let (read_req_tx, read_req_rx) = unbounded_channel();
    let (read_res_tx, read_res_rx) = unbounded_channel();
    let (events_req_tx, events_req_rx) = unbounded_channel();
    let (events_res_tx, events_res_rx) = unbounded_channel();
    let (storage_req_tx, storage_req_rx) = unbounded_channel();
    let (storage_res_tx, storage_res_rx) = unbounded_channel();
    (
        ContractFeed {
            input_tx,
            updates_rx,
            source_rx,
            read_tx: read_req_tx,
            read_rx: read_res_rx,
            events_tx: events_req_tx,
            events_rx: events_res_rx,
            storage_tx: storage_req_tx,
            storage_rx: storage_res_rx,
        },
        ContractFeedSender {
            updates_tx,
            source_tx,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractTab {
    Overview,
    Source,
    Abi,
    Read,
    Events,
    Storage,
}

impl ContractTab {
    const ALL: [ContractTab; 6] = [
        ContractTab::Overview,
        ContractTab::Source,
        ContractTab::Abi,
        ContractTab::Read,
        ContractTab::Events,
        ContractTab::Storage,
    ];

    fn next(self) -> Self {
        let idx = self.index();
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    fn previous(self) -> Self {
        let idx = self.index();
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    fn index(self) -> usize {
        match self {
            ContractTab::Overview => 0,
            ContractTab::Source => 1,
            ContractTab::Abi => 2,
            ContractTab::Read => 3,
            ContractTab::Events => 4,
            ContractTab::Storage => 5,
        }
    }

    fn label(self) -> &'static str {
        match self {
            ContractTab::Overview => "Overview",
            ContractTab::Source => "Source",
            ContractTab::Abi => "ABI",
            ContractTab::Read => "Read",
            ContractTab::Events => "Events",
            ContractTab::Storage => "Storage",
        }
    }
}

/// Focus within the Read tab: the function picker on the left, or
/// the argument editor on the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadFocus {
    FunctionList,
    Args,
}

pub struct ContractDetailScreen {
    #[allow(dead_code)]
    chain: Chain,
    address: Address,
    current: Option<ContractOverview>,
    source: Option<ContractSource>,
    feed: ContractFeed,
    active_tab: ContractTab,
    /// Bounded scroll shared by every tab whose body is rendered as
    /// a single `Paragraph` (plan 13.3, 12.6.6). The render path
    /// calls `set_dimensions` on every frame so `handle_key` can
    /// clamp the offset against the up-to-date content and
    /// viewport sizes.
    scroll: std::cell::Cell<ScrollState>,
    file_list_state: ListState,
    /// Read tab state.
    functions: Vec<AbiFunction>,
    function_list_state: ListState,
    read_focus: ReadFocus,
    /// One scratch buffer per input of the currently-selected
    /// function. Rebuilt when the selection changes.
    arg_buffers: Vec<String>,
    arg_cursor: usize,
    /// Rendered last result (or error) for the selected function.
    last_result: Option<Result<Vec<DecodedValue>, String>>,
    last_result_for: Option<String>,

    /// Events tab state.
    events: Option<Result<EventsPage, String>>,
    /// Pagination cursor. `offset = 0` is the newest window; each
    /// `n` press increments it, `Shift+N` decrements it. We store
    /// the head from the first page so further pages align with
    /// the same reference height.
    events_offset: u32,
    events_head: Option<BlockNumber>,
    events_requested: bool,

    /// Storage tab state. `slot_buffer` is the raw input string so
    /// the user can edit it; `storage_result` is the last read-out.
    slot_buffer: String,
    storage_result: Option<Result<[u8; 32], String>>,
    storage_slot_requested: Option<[u8; 32]>,
}

impl ContractDetailScreen {
    /// Build a screen in the loading state: emits one request on the
    /// feed and renders "Loading..." until the resolver task replies.
    #[must_use]
    pub fn loading(chain: Chain, address: Address, feed: ContractFeed) -> Self {
        let _ = feed.input_tx.send(address);
        let mut file_list_state = ListState::default();
        file_list_state.select(Some(0));
        let mut function_list_state = ListState::default();
        function_list_state.select(Some(0));
        Self {
            chain,
            address,
            current: None,
            source: None,
            feed,
            active_tab: ContractTab::Overview,
            scroll: std::cell::Cell::new(ScrollState::new()),
            file_list_state,
            functions: Vec::new(),
            function_list_state,
            read_focus: ReadFocus::FunctionList,
            arg_buffers: Vec::new(),
            arg_cursor: 0,
            last_result: None,
            last_result_for: None,
            events: None,
            events_offset: 0,
            events_head: None,
            events_requested: false,
            slot_buffer: "0".to_string(),
            storage_result: None,
            storage_slot_requested: None,
        }
    }

    #[must_use]
    pub fn current(&self) -> Option<&ContractOverview> {
        self.current.as_ref()
    }

    #[must_use]
    pub fn source(&self) -> Option<&ContractSource> {
        self.source.as_ref()
    }

    #[must_use]
    pub fn active_tab(&self) -> ContractTab {
        self.active_tab
    }

    /// Test helper: does the last Read-tab result hold a single
    /// uint with the given value?
    #[must_use]
    pub fn last_result_matches_uint(&self, expected: u128) -> bool {
        match self.last_result.as_ref() {
            Some(Ok(values)) if values.len() == 1 => {
                matches!(values[0], DecodedValue::Uint(v) if v == expected)
            }
            _ => false,
        }
    }

    /// Test helper: is the last Read-tab result an error whose
    /// message contains `needle`?
    #[must_use]
    pub fn last_result_is_error_containing(&self, needle: &str) -> bool {
        matches!(self.last_result.as_ref(), Some(Err(msg)) if msg.contains(needle))
    }

    /// Test helper: Events tab row count once loaded.
    #[must_use]
    pub fn events_count(&self) -> Option<usize> {
        match self.events.as_ref() {
            Some(Ok(page)) => Some(page.logs.len()),
            _ => None,
        }
    }

    /// Test helper: current Events tab page offset.
    #[must_use]
    pub fn events_offset(&self) -> u32 {
        self.events_offset
    }

    /// Test helper: loaded Events window as (from, to) block
    /// numbers. Returns `None` until the first page has arrived.
    #[must_use]
    pub fn events_window(&self) -> Option<(u64, u64)> {
        match self.events.as_ref() {
            Some(Ok(page)) => Some((page.window_from.value(), page.window_to.value())),
            _ => None,
        }
    }

    /// Test helper: Storage tab value parsed as a u128 (only valid
    /// when the high 16 bytes are zero).
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

    fn selected_file(&self) -> Option<&SourceFile> {
        let files = self.source.as_ref().map(|s| &s.files)?;
        let idx = self.file_list_state.selected().unwrap_or(0);
        files.get(idx)
    }

    fn drain_feed(&mut self) {
        while let Ok(update) = self.feed.updates_rx.try_recv() {
            self.current = Some(update);
        }
        while let Ok(source) = self.feed.source_rx.try_recv() {
            self.functions = parse_abi_functions(&source.abi);
            self.functions.sort_by(|a, b| a.name.cmp(&b.name));
            self.source = Some(source);
            self.clamp_file_selection();
            self.reset_args_for_current_fn();
        }
        while let Ok(result) = self.feed.read_rx.try_recv() {
            self.last_result = Some(result.map_err(|e| domain_error_message(&e)));
            self.last_result_for = self.selected_function().map(|f| f.signature());
        }
        while let Ok(result) = self.feed.events_rx.try_recv() {
            if let Ok(page) = result.as_ref() {
                // Cache the resolved head so the next page request
                // reuses it instead of re-running NetworkStatus.
                self.events_head = Some(page.head);
            }
            self.events = Some(result.map_err(|e| domain_error_message(&e)));
        }
        while let Ok(result) = self.feed.storage_rx.try_recv() {
            self.storage_result = Some(result.map_err(|e| domain_error_message(&e)));
        }
    }

    fn selected_function(&self) -> Option<&AbiFunction> {
        let idx = self.function_list_state.selected().unwrap_or(0);
        self.functions.get(idx)
    }

    fn reset_args_for_current_fn(&mut self) {
        let arg_count = self
            .selected_function()
            .map(|f| f.inputs.len())
            .unwrap_or(0);
        self.arg_buffers = vec![String::new(); arg_count];
        self.arg_cursor = 0;
        self.last_result = None;
        self.last_result_for = None;
    }

    fn select_function_delta(&mut self, delta: i32) {
        if self.functions.is_empty() {
            return;
        }
        let current = self.function_list_state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, self.functions.len() as i32 - 1);
        self.function_list_state.select(Some(next as usize));
        self.reset_args_for_current_fn();
    }

    /// Try to turn the user-supplied strings into [`AbiValue`]s for
    /// the currently-selected function. Returns `Err(msg)` with a
    /// user-visible explanation on the first failure.
    fn build_args(&self) -> Result<Vec<AbiValue>, String> {
        let function = self
            .selected_function()
            .ok_or_else(|| "no function selected".to_string())?;
        if function.inputs.len() != self.arg_buffers.len() {
            return Err(format!(
                "arg buffer mismatch ({} vs {})",
                function.inputs.len(),
                self.arg_buffers.len()
            ));
        }
        let mut values = Vec::with_capacity(function.inputs.len());
        for (param, raw) in function.inputs.iter().zip(self.arg_buffers.iter()) {
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

    fn clamp_file_selection(&mut self) {
        let len = self.source.as_ref().map(|s| s.files.len()).unwrap_or(0);
        if len == 0 {
            self.file_list_state.select(None);
            return;
        }
        let current = self.file_list_state.selected().unwrap_or(0);
        self.file_list_state.select(Some(current.min(len - 1)));
    }

    fn file_delta(&mut self, delta: i32) {
        let len = self.source.as_ref().map(|s| s.files.len()).unwrap_or(0);
        if len == 0 {
            return;
        }
        let current = self.file_list_state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1);
        self.file_list_state.select(Some(next as usize));
        self.with_scroll(|s| s.reset());
    }

    /// Mutate the inner [`ScrollState`] through the `Cell` without
    /// needing a `&mut self` borrow. Mirrors the helper in
    /// `TxDetailScreen`; used by both the render path (to refresh
    /// dimensions) and `handle_key` (to clamp the offset).
    fn with_scroll(&self, f: impl FnOnce(&mut ScrollState)) {
        let mut s = self.scroll.get();
        f(&mut s);
        self.scroll.set(s);
    }
}

impl Screen for ContractDetailScreen {
    fn title(&self) -> &str {
        "Contract"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(3),
            ])
            .split(area);

        // Header
        let header = match self.current.as_ref() {
            Some(ov) => {
                let verified = self
                    .source
                    .as_ref()
                    .map(|s| {
                        if s.is_verified {
                            "verified"
                        } else {
                            "unverified"
                        }
                    })
                    .unwrap_or("verification loading...");
                format!(
                    "Contract {addr}  [{verified}]",
                    addr = ov.account.address.to_hex(),
                )
            }
            None => "Contract (loading...)".to_string(),
        };
        frame.render_widget(
            Paragraph::new(header).block(Block::default().borders(Borders::ALL).title("Contract")),
            chunks[0],
        );

        // Tab bar
        let titles: Vec<Line<'static>> = ContractTab::ALL
            .iter()
            .map(|t| Line::from(format!(" {} ", t.label())))
            .collect();
        frame.render_widget(
            Tabs::new(titles)
                .select(self.active_tab.index())
                .block(Block::default().borders(Borders::ALL).title("Tabs"))
                .divider(" ")
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::Indexed(238))
                        .fg(Color::White),
                ),
            chunks[1],
        );

        // Body
        match self.active_tab {
            ContractTab::Overview => {
                let body = overview_body(self.address, self.current.as_ref(), self.source.as_ref());
                let offset = self.bound_scroll_for(&body, chunks[2]);
                frame.render_widget(
                    Paragraph::new(body)
                        .wrap(Wrap { trim: false })
                        .scroll((offset, 0))
                        .block(Block::default().borders(Borders::ALL).title("Overview")),
                    chunks[2],
                );
            }
            ContractTab::Source => self.render_source_tab(frame, chunks[2]),
            ContractTab::Abi => {
                let (title, body) = abi_body(self.source.as_ref());
                let offset = self.bound_scroll_for(&body, chunks[2]);
                frame.render_widget(
                    Paragraph::new(body)
                        .wrap(Wrap { trim: false })
                        .scroll((offset, 0))
                        .block(Block::default().borders(Borders::ALL).title(title)),
                    chunks[2],
                );
            }
            ContractTab::Read => self.render_read_tab(frame, chunks[2]),
            ContractTab::Events => self.render_events_tab(frame, chunks[2]),
            ContractTab::Storage => self.render_storage_tab(frame, chunks[2]),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        // Global keys first.
        match key.code {
            KeyCode::Char('q') => return Command::Quit,
            KeyCode::Esc => return Command::Pop,
            _ => {}
        }
        let is_back_tab = key.code == KeyCode::BackTab
            || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT));
        if is_back_tab {
            self.active_tab = self.active_tab.previous();
            self.with_scroll(|s| s.reset());
            return Command::None;
        }
        if key.code == KeyCode::Tab {
            self.active_tab = self.active_tab.next();
            self.with_scroll(|s| s.reset());
            return Command::None;
        }

        match self.active_tab {
            ContractTab::Source => self.handle_source_key(key),
            ContractTab::Read => self.handle_read_key(key),
            ContractTab::Events => self.handle_events_key(key),
            ContractTab::Storage => self.handle_storage_key(key),
            ContractTab::Overview | ContractTab::Abi => {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.with_scroll(|s| {
                            s.scroll_by(-1);
                        });
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.with_scroll(|s| {
                            s.scroll_by(1);
                        });
                    }
                    KeyCode::Left => {
                        self.active_tab = self.active_tab.previous();
                        self.with_scroll(|s| s.reset());
                    }
                    KeyCode::Right => {
                        self.active_tab = self.active_tab.next();
                        self.with_scroll(|s| s.reset());
                    }
                    KeyCode::PageUp => {
                        self.with_scroll(|s| {
                            s.page_up();
                        });
                    }
                    KeyCode::PageDown => {
                        self.with_scroll(|s| {
                            s.page_down();
                        });
                    }
                    KeyCode::Home => {
                        self.with_scroll(|s| {
                            s.home();
                        });
                    }
                    KeyCode::End => {
                        self.with_scroll(|s| {
                            s.end();
                        });
                    }
                    _ => {}
                }
                Command::None
            }
        }
    }

    fn tick(&mut self) -> Command {
        self.drain_feed();
        // Fire the first Events request lazily when the user actually
        // opens the tab; that way we never issue an eth_getLogs for
        // contracts the user just glances at.
        if self.active_tab == ContractTab::Events && !self.events_requested {
            let _ = self.feed.events_tx.send(EventsRequest {
                head_hint: self.events_head,
                offset: self.events_offset,
            });
            self.events_requested = true;
        }
        Command::None
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl ContractDetailScreen {
    fn handle_source_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Up => {
                self.file_delta(-1);
            }
            KeyCode::Down => {
                self.file_delta(1);
            }
            KeyCode::Char('k') => {
                self.with_scroll(|s| {
                    s.scroll_by(-1);
                });
            }
            KeyCode::Char('j') => {
                self.with_scroll(|s| {
                    s.scroll_by(1);
                });
            }
            KeyCode::PageUp => {
                self.with_scroll(|s| {
                    s.page_up();
                });
            }
            KeyCode::PageDown => {
                self.with_scroll(|s| {
                    s.page_down();
                });
            }
            KeyCode::Home => {
                self.with_scroll(|s| {
                    s.home();
                });
            }
            KeyCode::End => {
                self.with_scroll(|s| {
                    s.end();
                });
            }
            _ => {}
        }
        Command::None
    }

    fn handle_read_key(&mut self, key: KeyEvent) -> Command {
        match self.read_focus {
            ReadFocus::FunctionList => self.handle_read_list_key(key),
            ReadFocus::Args => self.handle_read_args_key(key),
        }
    }

    fn handle_read_list_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.select_function_delta(-1),
            KeyCode::Down | KeyCode::Char('j') => self.select_function_delta(1),
            KeyCode::PageUp => self.select_function_delta(-10),
            KeyCode::PageDown => self.select_function_delta(10),
            KeyCode::Home if !self.functions.is_empty() => {
                self.function_list_state.select(Some(0));
                self.reset_args_for_current_fn();
            }
            KeyCode::End if !self.functions.is_empty() => {
                let last = self.functions.len() - 1;
                self.function_list_state.select(Some(last));
                self.reset_args_for_current_fn();
            }
            KeyCode::Enter | KeyCode::Right => {
                // Focus the args editor. If the function has no
                // inputs, fire immediately.
                if let Some(f) = self.selected_function() {
                    if f.inputs.is_empty() {
                        self.execute_current();
                    } else {
                        self.read_focus = ReadFocus::Args;
                        self.arg_cursor = 0;
                    }
                }
            }
            _ => {}
        }
        Command::None
    }

    fn handle_read_args_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Up if self.arg_cursor > 0 => {
                self.arg_cursor -= 1;
            }
            KeyCode::Down if self.arg_cursor + 1 < self.arg_buffers.len() => {
                self.arg_cursor += 1;
            }
            KeyCode::Left => {
                self.read_focus = ReadFocus::FunctionList;
            }
            KeyCode::Backspace => {
                if let Some(buf) = self.arg_buffers.get_mut(self.arg_cursor) {
                    buf.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(buf) = self.arg_buffers.get_mut(self.arg_cursor) {
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

    fn execute_current(&mut self) {
        let Some(function) = self.selected_function().cloned() else {
            return;
        };
        if !function.is_executable() {
            self.last_result = Some(Err("function has unsupported ABI input types".to_string()));
            self.last_result_for = Some(function.signature());
            return;
        }
        match self.build_args() {
            Ok(args) => {
                let _ = self.feed.read_tx.send(ReadRequest { function, args });
            }
            Err(msg) => {
                self.last_result = Some(Err(msg));
                self.last_result_for = Some(function.signature());
            }
        }
    }

    fn render_read_tab(&self, frame: &mut Frame<'_>, area: Rect) {
        if self.source.is_none() {
            frame.render_widget(
                Paragraph::new("Loading ABI...")
                    .block(Block::default().borders(Borders::ALL).title("Read")),
                area,
            );
            return;
        }
        if self.functions.is_empty() {
            frame.render_widget(
                Paragraph::new(
                    "No callable functions in this ABI.\n\
Contract may be unverified or expose only events / constructors.",
                )
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title("Read")),
                area,
            );
            return;
        }

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);

        // Function list
        let items: Vec<ListItem> = self
            .functions
            .iter()
            .map(|f| {
                let marker = if f.is_read_only { " " } else { "!" };
                ListItem::new(format!("{marker} {}", f.signature()))
            })
            .collect();
        let mut state = self.function_list_state;
        let list_title = match self.read_focus {
            ReadFocus::FunctionList => "Functions (focused)",
            ReadFocus::Args => "Functions",
        };
        frame.render_stateful_widget(
            List::new(items)
                .block(Block::default().borders(Borders::ALL).title(list_title))
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::Indexed(238)),
                )
                .highlight_symbol("> "),
            columns[0],
            &mut state,
        );

        // Detail pane: split into metadata + args + result.
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
            Paragraph::new(meta_text)
                .block(Block::default().borders(Borders::ALL).title("Signature")),
            detail_chunks[0],
        );

        // Arguments editor
        let args_title = match self.read_focus {
            ReadFocus::Args => "Arguments (focused, [Enter] to execute)",
            ReadFocus::FunctionList => "Arguments ([Tab*] to focus)",
        };
        let args_body = match function {
            Some(f) if f.inputs.is_empty() => "(no arguments)".to_string(),
            Some(f) => {
                let mut lines = Vec::with_capacity(f.inputs.len());
                for (idx, (param, buf)) in f.inputs.iter().zip(self.arg_buffers.iter()).enumerate()
                {
                    let cursor =
                        if matches!(self.read_focus, ReadFocus::Args) && idx == self.arg_cursor {
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
            Paragraph::new(args_body)
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title(args_title)),
            detail_chunks[1],
        );

        // Result pane
        let result_body = match (
            self.last_result.as_ref(),
            self.last_result_for.as_deref(),
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
        frame.render_widget(
            Paragraph::new(result_body)
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title("Result")),
            detail_chunks[2],
        );
    }

    fn handle_events_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Char('r') | KeyCode::Enter => {
                // Force-refresh the current window. Head hint is
                // preserved so the refresh stays aligned with the
                // same chain height as the first load.
                self.events = None;
                let _ = self.feed.events_tx.send(EventsRequest {
                    head_hint: self.events_head,
                    offset: self.events_offset,
                });
            }
            // Plan 12.5.3: `n` pages to older blocks, only when the
            // current page has an older window available.
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
            // `N` (Shift+n) pages back towards the head. Floored at
            // 0 so the key is a no-op on the newest window.
            KeyCode::Char('N') if self.events_offset > 0 => {
                self.events_offset -= 1;
                self.events = None;
                let _ = self.feed.events_tx.send(EventsRequest {
                    head_hint: self.events_head,
                    offset: self.events_offset,
                });
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.with_scroll(|s| {
                    s.scroll_by(-1);
                });
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.with_scroll(|s| {
                    s.scroll_by(1);
                });
            }
            KeyCode::PageUp => {
                self.with_scroll(|s| {
                    s.page_up();
                });
            }
            KeyCode::PageDown => {
                self.with_scroll(|s| {
                    s.page_down();
                });
            }
            KeyCode::Home => {
                self.with_scroll(|s| {
                    s.home();
                });
            }
            KeyCode::End => {
                self.with_scroll(|s| {
                    s.end();
                });
            }
            _ => {}
        }
        Command::None
    }

    fn handle_storage_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Backspace => {
                self.slot_buffer.pop();
            }
            // Digits or hex chars only; everything else is ignored
            // so the input does not get polluted with stray keys.
            KeyCode::Char(c) if c.is_ascii_hexdigit() || c == 'x' || c == 'X' => {
                self.slot_buffer.push(c);
            }
            KeyCode::Enter => match parse_slot(&self.slot_buffer) {
                Ok(slot) => {
                    self.storage_slot_requested = Some(slot);
                    self.storage_result = None;
                    let _ = self.feed.storage_tx.send(StorageRequest { slot });
                }
                Err(msg) => {
                    self.storage_result = Some(Err(msg));
                }
            },
            _ => {}
        }
        Command::None
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
                .block(Block::default().borders(Borders::ALL).title("Events")),
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
        frame.render_widget(
            Paragraph::new(prompt).block(Block::default().borders(Borders::ALL).title("Slot")),
            chunks[0],
        );

        let body = match (self.storage_slot_requested, self.storage_result.as_ref()) {
            (Some(slot), Some(Ok(word))) => format_storage_word(slot, word),
            (Some(_), Some(Err(msg))) => format!("ERROR: {msg}"),
            (Some(_), None) => "Reading...".to_string(),
            (None, None) => "(press Enter to read the current slot)".to_string(),
            (None, Some(Err(msg))) => format!("ERROR: {msg}"),
            (None, Some(Ok(_))) => unreachable!(),
        };
        frame.render_widget(
            Paragraph::new(body)
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title("Value")),
            chunks[1],
        );
    }

    fn render_source_tab(&self, frame: &mut Frame<'_>, area: Rect) {
        let Some(source) = self.source.as_ref() else {
            frame.render_widget(
                Paragraph::new("Loading source...")
                    .block(Block::default().borders(Borders::ALL).title("Source")),
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
                .block(Block::default().borders(Borders::ALL).title("Source")),
                area,
            );
            return;
        }

        // Split: file picker on the left (30%), content on the right.
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        let items: Vec<ListItem> = source
            .files
            .iter()
            .map(|f| ListItem::new(f.path.clone()))
            .collect();
        let mut state = self.file_list_state;
        frame.render_stateful_widget(
            List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Files"))
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
        // For scroll-clamping we only need the line count, which
        // matches the highlighted output line-for-line. We pass the
        // raw string to `bound_scroll_for` so the existing helper
        // stays ignorant of the highlighter.
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
                .block(Block::default().borders(Borders::ALL).title(title)),
            columns[1],
        );
    }

    /// Refresh the scroll-state dimensions for the currently-
    /// rendered body and return the clamped offset to feed into
    /// `Paragraph::scroll((offset, 0))`. Plan 13.3 and 12.6.6.
    fn bound_scroll_for(&self, body: &str, area: Rect) -> u16 {
        let content_lines = body.lines().count() as u16;
        let viewport = area.height.saturating_sub(2);
        self.with_scroll(|s| s.set_dimensions(content_lines, viewport));
        self.scroll.get().offset()
    }
}

fn overview_body(
    address: Address,
    overview: Option<&ContractOverview>,
    source: Option<&ContractSource>,
) -> String {
    let Some(ov) = overview else {
        return "Loading...".to_string();
    };
    let proxy_line = match ov.proxy {
        Some(info) => {
            // For Transparent proxies the address stored in `info`
            // is actually the admin (plan/7 §12.5.2); label it so
            // the user is not misled into thinking it is the
            // implementation.
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
[Tab] cycle tabs    [Up/Down] pick file on Source    [PageUp/PageDown] scroll",
        addr = address.to_hex(),
        balance = ov.account.balance.value(),
        nonce = ov.account.nonce,
    )
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

    // Accept up to 64 hex digits (32 bytes). Zero-pad on the left.
    if radix == 16 {
        if digits.len() > 64 {
            return Err(format!("slot hex too long ({} > 64)", digits.len()));
        }
        let padded = format!("{:0>64}", digits);
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(&padded, &mut bytes).map_err(|e| format!("invalid hex: {e}"))?;
        Ok(bytes)
    } else {
        // Decimal: up to u128 (contracts with slots > 2^128 are rare
        // enough that we keep the code simple).
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
    // Best-effort pretty-print; fall back to the raw string if the
    // JSON parse fails.
    let pretty = serde_json::from_str::<serde_json::Value>(&source.abi)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| source.abi.clone());
    ("ABI".to_string(), pretty)
}
