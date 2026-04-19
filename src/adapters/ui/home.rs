//! Home screen ratatui widget.
//!
//! Two levels of API live here:
//!
//! * [`render`] is the pure renderer consumed by both the real runtime
//!   and the snapshot tests.
//! * [`HomeScreen`] is the [`Screen`] implementation driven by the
//!   dispatcher; it holds the [`HomeViewModel`] and maps key events to
//!   [`Command`]s.
//!
//! See `plan/1-home.md` section 2 for the target layout and
//! `plan/12-screen-runtime.md` section 2.4 for the screen contract.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};
use tokio::sync::mpsc::{
    UnboundedReceiver, UnboundedSender, error::TryRecvError, unbounded_channel,
};

use crate::{
    adapters::ui::screen::{Command, Screen},
    application::{ConnectionStatus, HomeViewModel},
    domain::{BlockNumber, Chain, GasSnapshot, Gwei, NetworkStatus, Wei},
};

/// Render the Home screen into `frame` at `area`.
pub fn render(frame: &mut Frame<'_>, area: Rect, view: &HomeViewModel) {
    render_with_banner(frame, area, view, false);
}

/// Variant of [`render`] that also paints a first-run credentials
/// banner above the header when `banner` is `true`. See
/// `plan/10-settings.md` section 12.2.
pub fn render_with_banner(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &HomeViewModel,
    banner: bool,
) {
    if banner {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(10),
            ])
            .split(area);
        render_first_run_banner(frame, chunks[0]);
        render_header(frame, chunks[1], view);
        render_cards(frame, chunks[2], view);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(10)])
            .split(area);
        render_header(frame, chunks[0], view);
        render_cards(frame, chunks[1], view);
    }
}

fn render_first_run_banner(frame: &mut Frame<'_>, area: Rect) {
    let body = "Set up credentials: press s to open Settings, or Esc / Enter to dismiss.";
    let para = Paragraph::new(body)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("No Alchemy key"),
        );
    frame.render_widget(para, area);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, view: &HomeViewModel) {
    let badge = match view.connection {
        ConnectionStatus::Connected => String::new(),
        ConnectionStatus::Disconnected {
            reconnect_scheduled,
        } => {
            if reconnect_scheduled {
                "  [disconnected, reconnecting]".to_string()
            } else {
                "  [disconnected]".to_string()
            }
        }
    };

    let text = format!("Chain: {}{}", view.chain.display_name(), badge);
    let para = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Home"))
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(para, area);
}

fn render_cards(frame: &mut Frame<'_>, area: Rect, view: &HomeViewModel) {
    let cards = Layout::default()
        .direction(if area.width >= 120 {
            Direction::Horizontal
        } else {
            Direction::Vertical
        })
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_network_card(frame, cards[0], view);
    render_gas_card(frame, cards[1], view);
}

fn render_network_card(frame: &mut Frame<'_>, area: Rect, view: &HomeViewModel) {
    let body = match view.network.as_ref() {
        Some(n) => format!(
            "Latest block   {}\nBlock time avg   {:.1}s\nBase fee   {} gwei",
            format_u64(n.latest_block.value()),
            n.block_time_avg_ms as f64 / 1000.0,
            n.base_fee.to_gwei().value(),
        ),
        None => "Loading network status...".to_string(),
    };
    let para = Paragraph::new(body)
        .alignment(Alignment::Left)
        .block(Block::default().borders(Borders::ALL).title("Network"));
    frame.render_widget(para, area);
}

fn render_gas_card(frame: &mut Frame<'_>, area: Rect, view: &HomeViewModel) {
    let body = match view.gas.as_ref() {
        Some(g) => format_gas(g),
        None => "Loading gas oracle...".to_string(),
    };
    let para = Paragraph::new(body)
        .alignment(Alignment::Left)
        .block(Block::default().borders(Borders::ALL).title("Gas Tracker"));
    frame.render_widget(para, area);
}

fn format_gas(g: &GasSnapshot) -> String {
    format!(
        "Slow    {} gwei\nAvg     {} gwei\nFast    {} gwei\nBase fee  {} gwei",
        g.slow.value(),
        g.average.value(),
        g.fast.value(),
        g.base_fee.value(),
    )
}

fn format_u64(n: u64) -> String {
    // Thousands separator using ASCII commas for terminal portability.
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*byte as char);
    }
    out
}

/// Receiving end of the channel that feeds the Home screen with fresh
/// view-model snapshots. Constructed via [`home_feed`].
pub struct HomeFeed {
    rx: UnboundedReceiver<HomeViewModel>,
}

impl HomeFeed {
    /// Await the next view-model update. Returns `None` when the
    /// sender has been dropped. Used by integration tests that drive
    /// the dispatcher directly (see
    /// `tests/functional/home_feed.rs::start_with_stream_*`).
    pub async fn recv(&mut self) -> Option<HomeViewModel> {
        self.rx.recv().await
    }
}

/// Sending end of the channel. Owned by the runtime's background
/// refresh task.
#[derive(Clone)]
pub struct HomeFeedSender {
    tx: UnboundedSender<HomeViewModel>,
}

impl HomeFeedSender {
    /// Push an update onto the feed. Errors mean the receiver was
    /// dropped (the screen is gone), in which case the runtime task
    /// should terminate. The returned boxed view is the one that
    /// failed to deliver, useful when callers want to log it.
    pub fn send(&self, view: HomeViewModel) -> Result<(), Box<HomeViewModel>> {
        self.tx.send(view).map_err(|e| Box::new(e.0))
    }
}

/// Build a new `(HomeFeed, HomeFeedSender)` pair.
#[must_use]
pub fn home_feed() -> (HomeFeed, HomeFeedSender) {
    let (tx, rx) = unbounded_channel();
    (HomeFeed { rx }, HomeFeedSender { tx })
}

/// Factory for the search screen, stored on the home screen so that
/// pressing `/` produces a ready-to-push `Box<dyn Screen>` without the
/// home screen knowing anything about the search adapters.
///
/// `None` means the `/` key is inert (used by demo mode, where there
/// is no resolver task to back the search screen).
pub type SearchFactory = Box<dyn Fn() -> Box<dyn crate::adapters::ui::Screen> + Send + 'static>;

/// Factory for the Mempool screen. Same rationale as
/// [`SearchFactory`]: keeps the home screen free of adapter details.
pub type MempoolFactory = Box<dyn Fn() -> Box<dyn crate::adapters::ui::Screen> + Send + 'static>;

/// Factory for the Gas Tracker screen.
pub type GasTrackerFactory = Box<dyn Fn() -> Box<dyn crate::adapters::ui::Screen> + Send + 'static>;

/// Factory for the Settings screen.
pub type SettingsFactory = Box<dyn Fn() -> Box<dyn crate::adapters::ui::Screen> + Send + 'static>;

/// Screen-level wrapper around [`render`].
///
/// Holds the current [`HomeViewModel`] and optionally a [`HomeFeed`]
/// that delivers updates from a background refresher task. See
/// `plan/14-config-and-credentials.md` section 3.
pub struct HomeScreen {
    view: HomeViewModel,
    feed: Option<HomeFeed>,
    search_factory: Option<SearchFactory>,
    mempool_factory: Option<MempoolFactory>,
    gas_factory: Option<GasTrackerFactory>,
    settings_factory: Option<SettingsFactory>,
    /// When `true`, render the first-run banner above the header.
    /// See `plan/10-settings.md` section 12.2.
    first_run_hint: bool,
}

impl HomeScreen {
    /// Build a `HomeScreen` that renders a fixed view model with no
    /// background updates.
    #[must_use]
    pub fn new(view: HomeViewModel) -> Self {
        Self {
            view,
            feed: None,
            search_factory: None,
            mempool_factory: None,
            gas_factory: None,
            settings_factory: None,
            first_run_hint: false,
        }
    }

    /// Build a `HomeScreen` that starts with `initial` and then
    /// absorbs updates delivered through `feed` on every tick.
    #[must_use]
    pub fn with_feed(initial: HomeViewModel, feed: HomeFeed) -> Self {
        Self {
            view: initial,
            feed: Some(feed),
            search_factory: None,
            mempool_factory: None,
            gas_factory: None,
            settings_factory: None,
            first_run_hint: false,
        }
    }

    /// Equip the home screen with a factory that produces a search
    /// screen when the user presses `/`. Returns `self` for chaining.
    #[must_use]
    pub fn with_search_factory(mut self, factory: SearchFactory) -> Self {
        self.search_factory = Some(factory);
        self
    }

    /// Equip the home screen with a factory that produces a Mempool
    /// screen when the user presses `m`.
    #[must_use]
    pub fn with_mempool_factory(mut self, factory: MempoolFactory) -> Self {
        self.mempool_factory = Some(factory);
        self
    }

    /// Equip the home screen with a factory that produces a Gas
    /// Tracker screen when the user presses `g`.
    #[must_use]
    pub fn with_gas_factory(mut self, factory: GasTrackerFactory) -> Self {
        self.gas_factory = Some(factory);
        self
    }

    /// Equip the home screen with a factory that produces a Settings
    /// screen when the user presses `s`.
    #[must_use]
    pub fn with_settings_factory(mut self, factory: SettingsFactory) -> Self {
        self.settings_factory = Some(factory);
        self
    }

    /// Enable or disable the first-run credentials banner. The binary
    /// sets it to `true` when no `ALCHEMY_API_KEY` is present and the
    /// user launched with `--demo` so they are routed to Settings →
    /// Credentials. See `plan/10-settings.md` section 12.2.
    #[must_use]
    pub fn with_first_run_hint(mut self, enabled: bool) -> Self {
        self.first_run_hint = enabled;
        self
    }

    /// Placeholder view-model used by `cargo run -- --demo` until the
    /// Alchemy adapter lands in phase 2 (`plan/13-alchemy-adapter.md`).
    /// Numbers are plausible but frozen in time.
    #[must_use]
    pub fn with_demo_data() -> Self {
        let view = HomeViewModel {
            chain: Chain::Ethereum,
            network: Some(NetworkStatus {
                chain: Chain::Ethereum,
                latest_block: BlockNumber::new(21_345_678),
                base_fee: Wei::new(11_400_000_000),
                block_time_avg_ms: 12_100,
            }),
            gas: Some(GasSnapshot {
                chain: Chain::Ethereum,
                slow: Gwei::new(12),
                average: Gwei::new(14),
                fast: Gwei::new(18),
                base_fee: Gwei::new(11),
                trend: (0..20).map(|i| Gwei::new(11 + (i % 5))).collect(),
            }),
            connection: ConnectionStatus::Connected,
        };
        Self::new(view)
    }

    /// Expose the inner view-model. Useful for tests.
    #[must_use]
    pub fn view(&self) -> &HomeViewModel {
        &self.view
    }
}

impl Screen for HomeScreen {
    fn title(&self) -> &str {
        "Home"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        render_with_banner(frame, area, &self.view, self.first_run_hint);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Command {
        match key.code {
            KeyCode::Char('q') => Command::Quit,
            KeyCode::Esc if self.first_run_hint => {
                self.first_run_hint = false;
                Command::None
            }
            KeyCode::Esc => Command::Pop,
            KeyCode::Enter if self.first_run_hint => {
                self.first_run_hint = false;
                Command::None
            }
            KeyCode::Char('/') => match self.search_factory.as_ref() {
                Some(factory) => Command::Push(factory()),
                None => Command::None,
            },
            KeyCode::Char('m') => match self.mempool_factory.as_ref() {
                Some(factory) => Command::Push(factory()),
                None => Command::None,
            },
            KeyCode::Char('g') => match self.gas_factory.as_ref() {
                Some(factory) => Command::Push(factory()),
                None => Command::None,
            },
            KeyCode::Char('s') => match self.settings_factory.as_ref() {
                Some(factory) => Command::Push(factory()),
                None => Command::None,
            },
            _ => Command::None,
        }
    }

    fn tick(&mut self) -> Command {
        if let Some(feed) = self.feed.as_mut() {
            loop {
                match feed.rx.try_recv() {
                    Ok(update) => self.view = update,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        // The sender is gone; stop trying.
                        self.feed = None;
                        break;
                    }
                }
            }
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
