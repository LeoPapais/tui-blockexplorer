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
    let backend = TestBackend::new(120, 20);
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
