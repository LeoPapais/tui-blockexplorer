//! Snapshot-style render tests for `HomeScreen`.
//!
//! Uses `ratatui::backend::TestBackend` to render a frame into an
//! in-memory buffer and then inspects the cell contents. See
//! `.cursor/rules/tui.mdc` for the convention.

use blockexplorer_tui::{
    adapters::ui::home,
    application::{ConnectionStatus, HomeViewModel},
    domain::Chain,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

use crate::support::stubs::{GasSnapshotFixture, NetworkStatusFixture};

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

fn render_to_buffer(view: &HomeViewModel) -> Buffer {
    render_to_buffer_with_size(view, 120, 20)
}

fn render_to_buffer_with_size(view: &HomeViewModel, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| home::render(frame, frame.area(), view))
        .expect("draw");
    terminal.backend().buffer().clone()
}

#[test]
fn renders_chain_name_latest_block_and_gas_tiers() {
    let view = HomeViewModel {
        chain: Chain::Ethereum,
        network: Some(NetworkStatusFixture::load(
            "home__network_status__ethereum.json",
        )),
        gas: Some(GasSnapshotFixture::load(
            "home__gas_snapshot__ethereum.json",
        )),
        connection: ConnectionStatus::Connected,
    };

    let buffer = render_to_buffer(&view);

    assert!(buffer_contains(&buffer, "Chain: Ethereum"));
    assert!(buffer_contains(&buffer, "21,345,678"));
    assert!(buffer_contains(&buffer, "Slow"));
    assert!(buffer_contains(&buffer, "Avg"));
    assert!(buffer_contains(&buffer, "Fast"));
    assert!(buffer_contains(&buffer, "14 gwei"));
}

#[test]
fn renders_disconnected_badge_when_connection_dropped() {
    let view = HomeViewModel {
        chain: Chain::Ethereum,
        network: Some(NetworkStatusFixture::load(
            "home__network_status__ethereum.json",
        )),
        gas: Some(GasSnapshotFixture::load(
            "home__gas_snapshot__ethereum.json",
        )),
        connection: ConnectionStatus::Disconnected {
            reconnect_scheduled: true,
        },
    };

    let buffer = render_to_buffer(&view);

    assert!(buffer_contains(&buffer, "disconnected"));
    assert!(buffer_contains(&buffer, "reconnecting"));
}

/// Narrow-terminal fallback (plan/1-home.md §2): when the terminal is
/// below the 120-column threshold the two cards stack vertically. This
/// test pins the stacked layout by rendering into a 60×30 TestBackend
/// and asserting that every content line from both cards is still
/// visible on its own row. It also asserts that the "Network" card title
/// appears above the "Gas Tracker" one, which is only true in the
/// vertical layout.
#[test]
fn renders_narrow_terminal_fallback_with_stacked_cards() {
    let view = HomeViewModel {
        chain: Chain::Ethereum,
        network: Some(NetworkStatusFixture::load(
            "home__network_status__ethereum.json",
        )),
        gas: Some(GasSnapshotFixture::load(
            "home__gas_snapshot__ethereum.json",
        )),
        connection: ConnectionStatus::Connected,
    };

    let buffer = render_to_buffer_with_size(&view, 60, 30);

    assert!(
        buffer_contains(&buffer, "Chain: Ethereum"),
        "header must render on narrow terminals"
    );
    assert!(
        buffer_contains(&buffer, "21,345,678"),
        "Network card must still show the latest block number"
    );
    assert!(
        buffer_contains(&buffer, "Slow"),
        "Gas Tracker card must still show the Slow tier"
    );
    assert!(
        buffer_contains(&buffer, "Fast"),
        "Gas Tracker card must still show the Fast tier"
    );

    let network_row = row_of(&buffer, "Network").expect("Network card borders");
    let gas_row = row_of(&buffer, "Gas Tracker").expect("Gas Tracker card borders");
    assert!(
        network_row < gas_row,
        "narrow layout must stack vertically: Network ({network_row}) must appear above Gas Tracker ({gas_row})"
    );
}

/// When the backend drops but the session keeps its last good snapshots,
/// the cards must still render them instead of the initial "Loading..."
/// placeholder. Covers plan/15-backlog.md §8.2 "connection-drop degraded
/// state" from the render side.
#[test]
fn renders_cached_snapshots_while_reconnecting() {
    let view = HomeViewModel {
        chain: Chain::Ethereum,
        network: Some(NetworkStatusFixture::load(
            "home__network_status__ethereum.json",
        )),
        gas: Some(GasSnapshotFixture::load(
            "home__gas_snapshot__ethereum.json",
        )),
        connection: ConnectionStatus::Disconnected {
            reconnect_scheduled: true,
        },
    };

    let buffer = render_to_buffer(&view);

    assert!(
        buffer_contains(&buffer, "reconnecting"),
        "the reconnecting hint must live on the header"
    );
    assert!(
        buffer_contains(&buffer, "21,345,678"),
        "the Network card must still show the last-known block number"
    );
    assert!(
        buffer_contains(&buffer, "Slow"),
        "the Gas Tracker card must still show the Slow tier"
    );
    assert!(
        !buffer_contains(&buffer, "Loading network"),
        "must not regress to the initial loading placeholder"
    );
    assert!(
        !buffer_contains(&buffer, "Loading gas"),
        "must not regress to the initial gas placeholder"
    );
}

/// Return the first row index in which `needle` is visible. `None` when
/// the needle is not present at all.
fn row_of(buffer: &Buffer, needle: &str) -> Option<u16> {
    let mut row = String::new();
    for y in 0..buffer.area.height {
        row.clear();
        for x in 0..buffer.area.width {
            row.push_str(buffer[(x, y)].symbol());
        }
        if row.contains(needle) {
            return Some(y);
        }
    }
    None
}
