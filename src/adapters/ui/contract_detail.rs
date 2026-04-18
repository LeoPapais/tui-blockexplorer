//! Contract Detail screen.
//!
//! Tabs: Overview, Source, ABI. Read / Events / Storage land in the
//! follow-up commits of the plan-7 expansion. See
//! `plan/7-contract-detail.md` sections 12.3 (MVP) and 12.4.1.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    adapters::ui::screen::{Command, Screen},
    domain::{Address, Chain, ContractOverview, ContractSource, SourceFile},
};

pub struct ContractFeed {
    pub input_tx: UnboundedSender<Address>,
    pub updates_rx: UnboundedReceiver<ContractOverview>,
    pub source_rx: UnboundedReceiver<ContractSource>,
}

pub struct ContractFeedSender {
    pub updates_tx: UnboundedSender<ContractOverview>,
    pub source_tx: UnboundedSender<ContractSource>,
    pub input_rx: UnboundedReceiver<Address>,
}

#[must_use]
pub fn contract_feed() -> (ContractFeed, ContractFeedSender) {
    let (input_tx, input_rx) = unbounded_channel();
    let (updates_tx, updates_rx) = unbounded_channel();
    let (source_tx, source_rx) = unbounded_channel();
    (
        ContractFeed {
            input_tx,
            updates_rx,
            source_rx,
        },
        ContractFeedSender {
            updates_tx,
            source_tx,
            input_rx,
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractTab {
    Overview,
    Source,
    Abi,
}

impl ContractTab {
    const ALL: [ContractTab; 3] = [ContractTab::Overview, ContractTab::Source, ContractTab::Abi];

    fn next(self) -> Self {
        let idx = self.index();
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    fn index(self) -> usize {
        match self {
            ContractTab::Overview => 0,
            ContractTab::Source => 1,
            ContractTab::Abi => 2,
        }
    }

    fn label(self) -> &'static str {
        match self {
            ContractTab::Overview => "Overview",
            ContractTab::Source => "Source",
            ContractTab::Abi => "ABI",
        }
    }
}

pub struct ContractDetailScreen {
    #[allow(dead_code)]
    chain: Chain,
    address: Address,
    current: Option<ContractOverview>,
    source: Option<ContractSource>,
    feed: ContractFeed,
    active_tab: ContractTab,
    scroll: u16,
    file_list_state: ListState,
}

impl ContractDetailScreen {
    /// Build a screen in the loading state: emits one request on the
    /// feed and renders "Loading..." until the resolver task replies.
    #[must_use]
    pub fn loading(chain: Chain, address: Address, feed: ContractFeed) -> Self {
        let _ = feed.input_tx.send(address);
        let mut file_list_state = ListState::default();
        file_list_state.select(Some(0));
        Self {
            chain,
            address,
            current: None,
            source: None,
            feed,
            active_tab: ContractTab::Overview,
            scroll: 0,
            file_list_state,
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
            self.source = Some(source);
            self.clamp_file_selection();
        }
    }

    fn clamp_file_selection(&mut self) {
        let len = self
            .source
            .as_ref()
            .map(|s| s.files.len())
            .unwrap_or(0);
        if len == 0 {
            self.file_list_state.select(None);
            return;
        }
        let current = self.file_list_state.selected().unwrap_or(0);
        self.file_list_state.select(Some(current.min(len - 1)));
    }

    fn file_delta(&mut self, delta: i32) {
        let len = self
            .source
            .as_ref()
            .map(|s| s.files.len())
            .unwrap_or(0);
        if len == 0 {
            return;
        }
        let current = self.file_list_state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1);
        self.file_list_state.select(Some(next as usize));
        self.scroll = 0;
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
                    .map(|s| if s.is_verified { "verified" } else { "unverified" })
                    .unwrap_or("verification loading...");
                format!(
                    "Contract {addr}  [{verified}]",
                    addr = ov.account.address.to_hex(),
                )
            }
            None => "Contract (loading...)".to_string(),
        };
        frame.render_widget(
            Paragraph::new(header).block(
                Block::default().borders(Borders::ALL).title("Contract"),
            ),
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
            ContractTab::Overview => frame.render_widget(
                Paragraph::new(overview_body(self.address, self.current.as_ref(), self.source.as_ref()))
                    .wrap(Wrap { trim: false })
                    .scroll((self.scroll, 0))
                    .block(Block::default().borders(Borders::ALL).title("Overview")),
                chunks[2],
            ),
            ContractTab::Source => self.render_source_tab(frame, chunks[2]),
            ContractTab::Abi => {
                let (title, body) = abi_body(self.source.as_ref());
                frame.render_widget(
                    Paragraph::new(body)
                        .wrap(Wrap { trim: false })
                        .scroll((self.scroll, 0))
                        .block(Block::default().borders(Borders::ALL).title(title)),
                    chunks[2],
                );
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        match (self.active_tab, key.code) {
            (_, KeyCode::Char('q')) => Command::Quit,
            (_, KeyCode::Esc) => Command::Pop,
            (_, KeyCode::Tab | KeyCode::BackTab) => {
                self.active_tab = self.active_tab.next();
                self.scroll = 0;
                Command::None
            }

            // Source tab: Up/Down switch file, PageUp/PageDown scroll.
            (ContractTab::Source, KeyCode::Up) => {
                self.file_delta(-1);
                Command::None
            }
            (ContractTab::Source, KeyCode::Down) => {
                self.file_delta(1);
                Command::None
            }
            (ContractTab::Source, KeyCode::Char('k')) => {
                self.scroll = self.scroll.saturating_sub(1);
                Command::None
            }
            (ContractTab::Source, KeyCode::Char('j')) => {
                self.scroll = self.scroll.saturating_add(1);
                Command::None
            }
            (ContractTab::Source, KeyCode::PageUp) => {
                self.scroll = self.scroll.saturating_sub(10);
                Command::None
            }
            (ContractTab::Source, KeyCode::PageDown) => {
                self.scroll = self.scroll.saturating_add(10);
                Command::None
            }
            (ContractTab::Source, KeyCode::Home) => {
                self.scroll = 0;
                Command::None
            }
            (ContractTab::Source, KeyCode::End) => {
                self.scroll = u16::MAX;
                Command::None
            }

            // Overview + ABI: body scroll
            (_, KeyCode::Up | KeyCode::Char('k')) => {
                self.scroll = self.scroll.saturating_sub(1);
                Command::None
            }
            (_, KeyCode::Down | KeyCode::Char('j')) => {
                self.scroll = self.scroll.saturating_add(1);
                Command::None
            }
            (_, KeyCode::PageUp) => {
                self.scroll = self.scroll.saturating_sub(10);
                Command::None
            }
            (_, KeyCode::PageDown) => {
                self.scroll = self.scroll.saturating_add(10);
                Command::None
            }
            (_, KeyCode::Home) => {
                self.scroll = 0;
                Command::None
            }
            (_, KeyCode::End) => {
                self.scroll = u16::MAX;
                Command::None
            }
            _ => Command::None,
        }
    }

    fn tick(&mut self) -> Command {
        self.drain_feed();
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
    fn render_source_tab(&self, frame: &mut Frame<'_>, area: Rect) {
        let Some(source) = self.source.as_ref() else {
            frame.render_widget(
                Paragraph::new("Loading source...").block(
                    Block::default().borders(Borders::ALL).title("Source"),
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
        let content = file.map(|f| f.content.clone()).unwrap_or_default();
        frame.render_widget(
            Paragraph::new(content)
                .wrap(Wrap { trim: false })
                .scroll((self.scroll, 0))
                .block(Block::default().borders(Borders::ALL).title(title)),
            columns[1],
        );
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
        Some(info) => format!(
            "Proxy       {kind} -> {impl_addr}",
            kind = info.kind.label(),
            impl_addr = info.implementation.to_hex(),
        ),
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

fn abi_body(source: Option<&ContractSource>) -> (String, String) {
    let Some(source) = source else {
        return ("ABI".to_string(), "Loading ABI...".to_string());
    };
    if !source.is_verified || source.abi.trim().is_empty() {
        return ("ABI".to_string(), "No ABI available (contract unverified).".to_string());
    }
    // Best-effort pretty-print; fall back to the raw string if the
    // JSON parse fails.
    let pretty = serde_json::from_str::<serde_json::Value>(&source.abi)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| source.abi.clone());
    ("ABI".to_string(), pretty)
}
