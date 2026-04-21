//! TUI event loop.
//!
//! Handles terminal setup/teardown, keyboard input, the periodic tick
//! and the main dispatch loop. Screen-agnostic: consumes any
//! [`ScreenStack`] and optionally a [`GlobalKeyMap`] so the `/` and
//! `?` bindings work on every screen.
//!
//! See `plan/12-screen-runtime.md` §3 + §7.

use std::{
    io::Stdout,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEvent, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use tokio::{signal, sync::mpsc};

use crate::adapters::ui::{Command, GlobalKeyMap, Screen, ScreenStack, render_breadcrumb};

/// Events driving the dispatcher.
#[derive(Debug)]
enum AppEvent {
    Key(KeyEvent),
    Tick,
    /// SIGINT (Ctrl+C) caught via `tokio::signal::ctrl_c`. Dispatched
    /// as a `Command::Quit` inside the main loop.
    Interrupt,
}

/// Tick period for the `AppEvent::Tick` stream. Matches the default used
/// across the plan files.
const TICK_PERIOD: Duration = Duration::from_millis(250);

/// Input polling timeout for the blocking read loop. Kept well below the
/// tick period so the UI stays responsive.
const INPUT_POLL_TIMEOUT: Duration = Duration::from_millis(100);

type Tui = Terminal<CrosstermBackend<Stdout>>;

/// One-shot guard used to ensure the terminal is torn down at most
/// once. Shared by the panic hook and the regular exit path so `q`
/// after a panic during startup still behaves correctly.
#[derive(Debug, Default)]
pub struct TeardownGate {
    fired: AtomicBool,
}

impl TeardownGate {
    /// Build a fresh gate (nothing fired yet).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fired: AtomicBool::new(false),
        }
    }

    /// Claim the gate. Returns `true` for the first caller, `false`
    /// for every subsequent one.
    pub fn claim(&self) -> bool {
        self.fired
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// `true` once any caller has claimed the gate.
    #[must_use]
    pub fn fired(&self) -> bool {
        self.fired.load(Ordering::Acquire)
    }
}

/// Map a `Ctrl+C` signal to the dispatcher command. Pulled out as a
/// pure function so it can be unit-tested without actually sending a
/// signal.
#[must_use]
pub const fn signal_to_command() -> Command {
    Command::Quit
}

/// Run the event loop until the top screen asks to quit or the stack
/// becomes empty.
pub async fn run_event_loop(stack: ScreenStack) -> Result<()> {
    run_event_loop_with_keymap(stack, GlobalKeyMap::default()).await
}

/// Same as [`run_event_loop`] but with a caller-supplied
/// [`GlobalKeyMap`]. The keymap is consulted *before* the current
/// screen's `handle_key`: `/` and `?` open modals from anywhere.
pub async fn run_event_loop_with_keymap(
    mut stack: ScreenStack,
    keymap: GlobalKeyMap,
) -> Result<()> {
    // Install the panic hook *before* switching the terminal into raw
    // mode so a panic during `enter_tui` still restores the terminal.
    let gate = Arc::new(TeardownGate::new());
    install_panic_hook(Arc::clone(&gate));

    let mut terminal = enter_tui().context("failed to enter TUI")?;

    let result = drive_loop(&mut terminal, &mut stack, &keymap).await;

    // Idempotent teardown: if the panic hook already claimed the gate
    // we skip the second disable_raw_mode so crossterm does not
    // double-swap state. See `plan/12-screen-runtime.md` §7 item 2.
    if gate.claim() {
        leave_tui(&mut terminal).context("failed to leave TUI")?;
    }
    result
}

async fn drive_loop(
    terminal: &mut Tui,
    stack: &mut ScreenStack,
    keymap: &GlobalKeyMap,
) -> Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();

    // Input task: blocking poll inside `spawn_blocking`.
    let input_tx = tx.clone();
    let input_handle = tokio::task::spawn_blocking(move || input_loop(&input_tx));

    // Ticker task.
    let ticker_tx = tx.clone();
    let ticker_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(TICK_PERIOD);
        // Drop the first tick so we do not immediately race with the
        // initial render.
        interval.tick().await;
        loop {
            interval.tick().await;
            if ticker_tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    });

    // Ctrl+C task: `tokio::signal::ctrl_c()` completes once per
    // SIGINT. We translate every hit into a single `AppEvent::Interrupt`
    // and loop so the user can fire Ctrl+C again after dismissing a
    // modal. See `plan/12-screen-runtime.md` §7 item 1.
    let signal_tx = tx;
    let signal_handle = tokio::spawn(async move {
        loop {
            if signal::ctrl_c().await.is_err() {
                break;
            }
            if signal_tx.send(AppEvent::Interrupt).is_err() {
                break;
            }
        }
    });

    // Initial draw before any event is processed.
    redraw(terminal, stack)?;

    while let Some(event) = rx.recv().await {
        let cmd = match event {
            AppEvent::Key(key) => dispatch_key(stack, keymap, key),
            AppEvent::Tick => dispatch_tick(stack),
            AppEvent::Interrupt => signal_to_command(),
        };

        let transition = stack.apply_command(cmd);
        if transition.should_exit() {
            break;
        }

        redraw(terminal, stack)?;
    }

    input_handle.abort();
    ticker_handle.abort();
    signal_handle.abort();
    Ok(())
}

/// Dispatch a key press: modal first, then global keymap, then the
/// screen at the top of the stack.
fn dispatch_key(stack: &mut ScreenStack, keymap: &GlobalKeyMap, key: KeyEvent) -> Command {
    if let Some(modal) = stack.modal_mut() {
        return modal.handle_key(key);
    }
    // Keymap gets a crack at the key before the screen does so `/`
    // and `?` work even when the screen has its own binding for the
    // same key.
    match keymap.dispatch(key) {
        Command::None => stack
            .top_mut()
            .map_or(Command::Quit, |top| top.handle_key(key)),
        other => other,
    }
}

fn dispatch_tick(stack: &mut ScreenStack) -> Command {
    // Tick the modal first, then the screen. Modal tick commands
    // take precedence; screen ticks only fire when the modal returns
    // `None`.
    if let Some(modal) = stack.modal_mut() {
        let cmd = modal.tick();
        if !matches!(cmd, Command::None) {
            return cmd;
        }
    }
    stack.top_mut().map_or(Command::None, |top| top.tick())
}

fn redraw(terminal: &mut Tui, stack: &ScreenStack) -> Result<()> {
    if let Some(top) = stack.top() {
        terminal.draw(|frame| {
            let area = frame.area();
            if stack.modal().is_some() {
                // Modal case: screen owns the full area underneath,
                // modal draws on top. Matches pre-footer behaviour so
                // the search overlay still paints its own input row.
                top.render(frame, area);
                if let Some(modal) = stack.modal() {
                    modal.render(frame, area);
                }
            } else {
                draw_screen_with_footer(frame, area, top, stack);
            }
        })?;
    }
    Ok(())
}

/// Render `screen` into `area`, reserving the bottom row for the
/// screen's footer hints. Used by the runtime's redraw path and by
/// footer render tests. See `plan/15-backlog.md` §8.13 and the TUI
/// rules in `.cursor/rules/tui.mdc`.
pub fn draw_screen_with_footer(
    frame: &mut Frame<'_>,
    area: Rect,
    screen: &dyn Screen,
    stack: &ScreenStack,
) {
    if area.height < 3 {
        let (body, footer) = split_for_footer(area);
        screen.render(frame, body);
        render_footer(frame, footer, screen);
        return;
    }

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    let trail = render_breadcrumb(stack);
    if !trail.is_empty() {
        frame.render_widget(
            Paragraph::new(trail).style(Style::default().fg(Color::DarkGray)),
            vertical[0],
        );
    }

    let (body, footer) = split_for_footer(vertical[1]);
    screen.render(frame, body);
    render_footer(frame, footer, screen);
}

/// Split `area` into a `(body, footer)` pair: the footer is a
/// single-row strip pinned to the bottom. When `area` has height
/// `0` or `1` the footer is returned as zero-row and the body
/// absorbs the whole rectangle — tiny terminals fall back to the
/// pre-footer layout. See `plan/15-backlog.md` §8.13.
#[must_use]
pub fn split_for_footer(area: Rect) -> (Rect, Rect) {
    if area.height <= 1 {
        return (area, Rect::new(area.x, area.bottom(), area.width, 0));
    }
    let body = Rect {
        height: area.height - 1,
        ..area
    };
    let footer = Rect {
        x: area.x,
        y: area.bottom() - 1,
        width: area.width,
        height: 1,
    };
    (body, footer)
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, screen: &dyn Screen) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let mut hints: Vec<(&'static str, &'static str)> = Vec::new();
    for pair in [("/", "Search"), ("?", "Help"), ("Esc", "Back")] {
        hints.push(pair);
    }
    for pair in screen.footer_hints() {
        if !hints.iter().any(|(k, _)| *k == pair.0) {
            hints.push(pair);
        }
    }
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(hints.len() * 3);
    for (i, (key, action)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            format!("[{key}]"),
            Style::default().add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            (*action).to_string(),
            Style::default().add_modifier(Modifier::DIM),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn input_loop(tx: &mpsc::UnboundedSender<AppEvent>) {
    loop {
        match event::poll(INPUT_POLL_TIMEOUT) {
            Ok(true) => {
                let Ok(evt) = event::read() else { continue };
                if let Event::Key(key) = evt
                    && key.kind == KeyEventKind::Press
                    && tx.send(AppEvent::Key(key)).is_err()
                {
                    break;
                }
            }
            Ok(false) => continue,
            Err(_) => break,
        }
    }
}

fn enter_tui() -> Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn leave_tui(terminal: &mut Tui) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Install a panic hook that restores the terminal exactly once.
/// Shares the [`TeardownGate`] with the regular exit path so the
/// two never fight over raw mode. Must run *before* `enter_tui`.
fn install_panic_hook(gate: Arc<TeardownGate>) {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if gate.claim() {
            let _ = disable_raw_mode();
            let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        }
        original(info);
    }));
}
