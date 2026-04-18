//! Snapshot-style render tests for [`MempoolScreen`].
//!
//! Mirrors `home_screen_render.rs`. Pins the three connection-state
//! badges and the paused badge so we notice any accidental rewording
//! of the UX strings from `plan/5-mempool.md` §11.3.3.

use blockexplorer_tui::{
    adapters::ui::{MempoolScreen, Screen},
    application::ConnectionStatus,
    domain::{Address, PendingTx, PendingTxEvent, PendingTxFilter, TxHash, Wei},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use tokio::sync::mpsc::unbounded_channel;

fn buffer_contains(buffer: &Buffer, needle: &str) -> bool {
    let mut row = String::new();
    for y in 0..buffer.area.height {
        row.clear();
        for x in 0..buffer.area.width {
            row.push_str(buffer[(x, y)].symbol());
        }
        if row.contains(needle) {
            return true;
        }
    }
    false
}

fn render_to_buffer(screen: &MempoolScreen) -> Buffer {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| screen.render(frame, frame.area()))
        .expect("draw");
    terminal.backend().buffer().clone()
}

fn sample_pending() -> PendingTx {
    PendingTx {
        hash: TxHash::from_hex(
            "0xaaaa000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap(),
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()),
        value: Wei::new(1_000),
    }
}

fn open_tx_panic() -> Box<dyn Fn(TxHash) -> Box<dyn Screen> + Send> {
    Box::new(|_| panic!("tx navigation not exercised"))
}

#[test]
fn connected_state_renders_no_status_badge() {
    let (_tx, rx) = unbounded_channel::<PendingTxEvent>();
    let screen = MempoolScreen::new(rx, PendingTxFilter::default(), open_tx_panic());
    let buffer = render_to_buffer(&screen);

    assert!(buffer_contains(&buffer, "Mempool"));
    assert!(!buffer_contains(&buffer, "[reconnecting]"));
    assert!(!buffer_contains(&buffer, "[disconnected]"));
}

#[test]
fn reconnecting_state_renders_reconnecting_badge() {
    let (_tx, rx) = unbounded_channel::<PendingTxEvent>();
    let (status_tx, status_rx) = unbounded_channel::<ConnectionStatus>();
    let mut screen = MempoolScreen::new(rx, PendingTxFilter::default(), open_tx_panic())
        .with_status_feed(status_rx);
    status_tx
        .send(ConnectionStatus::Disconnected {
            reconnect_scheduled: true,
        })
        .unwrap();
    screen.tick();
    let buffer = render_to_buffer(&screen);

    assert!(buffer_contains(&buffer, "[reconnecting]"));
}

#[test]
fn disconnected_without_reconnect_renders_disconnected_badge() {
    let (_tx, rx) = unbounded_channel::<PendingTxEvent>();
    let (status_tx, status_rx) = unbounded_channel::<ConnectionStatus>();
    let mut screen = MempoolScreen::new(rx, PendingTxFilter::default(), open_tx_panic())
        .with_status_feed(status_rx);
    status_tx
        .send(ConnectionStatus::Disconnected {
            reconnect_scheduled: false,
        })
        .unwrap();
    screen.tick();
    let buffer = render_to_buffer(&screen);

    assert!(buffer_contains(&buffer, "[disconnected]"));
    assert!(!buffer_contains(&buffer, "[reconnecting]"));
}

#[test]
fn paused_and_reconnecting_badges_coexist() {
    let (events_tx, events_rx) = unbounded_channel();
    let (status_tx, status_rx) = unbounded_channel();
    let mut screen = MempoolScreen::new(events_rx, PendingTxFilter::default(), open_tx_panic())
        .with_status_feed(status_rx);

    events_tx
        .send(PendingTxEvent::Added(sample_pending()))
        .unwrap();
    screen.tick();

    screen.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
    status_tx
        .send(ConnectionStatus::Disconnected {
            reconnect_scheduled: true,
        })
        .unwrap();
    screen.tick();

    let buffer = render_to_buffer(&screen);
    assert!(buffer_contains(&buffer, "[paused]"));
    assert!(buffer_contains(&buffer, "[reconnecting]"));
    assert_eq!(screen.items().len(), 1);
}
