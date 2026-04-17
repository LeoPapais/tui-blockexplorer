//! TUI event loop.
//!
//! Handles terminal setup/teardown, keyboard input, the periodic tick and
//! the main dispatch loop. Screen-agnostic: consumes any [`ScreenStack`].
//!
//! See `plan/12-screen-runtime.md` sections 3 and 4.

use std::{io::Stdout, time::Duration};

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEvent, KeyEventKind},
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;

use crate::adapters::ui::{Command, ScreenStack};

/// Events driving the dispatcher.
#[derive(Debug)]
enum AppEvent {
    Key(KeyEvent),
    Tick,
}

/// Tick period for the `AppEvent::Tick` stream. Matches the default used
/// across the plan files.
const TICK_PERIOD: Duration = Duration::from_millis(250);

/// Input polling timeout for the blocking read loop. Kept well below the
/// tick period so the UI stays responsive.
const INPUT_POLL_TIMEOUT: Duration = Duration::from_millis(100);

type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Run the event loop until the top screen asks to quit or the stack
/// becomes empty.
pub async fn run_event_loop(mut stack: ScreenStack) -> Result<()> {
    let mut terminal = enter_tui().context("failed to enter TUI")?;
    install_panic_hook();

    let result = drive_loop(&mut terminal, &mut stack).await;

    leave_tui(&mut terminal).context("failed to leave TUI")?;
    result
}

async fn drive_loop(terminal: &mut Tui, stack: &mut ScreenStack) -> Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();

    // Input task: blocking poll inside `spawn_blocking`.
    let input_tx = tx.clone();
    let input_handle = tokio::task::spawn_blocking(move || input_loop(&input_tx));

    // Ticker task.
    let ticker_tx = tx;
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

    // Initial draw before any event is processed.
    redraw(terminal, stack)?;

    while let Some(event) = rx.recv().await {
        let cmd = match event {
            AppEvent::Key(key) => {
                if let Some(top) = stack.top_mut() {
                    top.handle_key(key)
                } else {
                    Command::Quit
                }
            }
            AppEvent::Tick => {
                if let Some(top) = stack.top_mut() {
                    top.tick()
                } else {
                    Command::None
                }
            }
        };

        match cmd {
            Command::None | Command::Refresh => {}
            Command::Pop => {
                stack.pop();
                if stack.is_empty() {
                    break;
                }
            }
            Command::Quit => {
                stack.clear();
                break;
            }
        }

        redraw(terminal, stack)?;
    }

    input_handle.abort();
    ticker_handle.abort();
    Ok(())
}

fn redraw(terminal: &mut Tui, stack: &ScreenStack) -> Result<()> {
    if let Some(top) = stack.top() {
        terminal.draw(|frame| top.render(frame, frame.area()))?;
    }
    Ok(())
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

/// Make sure the terminal is restored even if a panic unwinds through
/// the event loop.
fn install_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(
            std::io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        hook(info);
    }));
}
