//! Snapshot tests for the Search overlay layout.
//!
//! Mirrors the runtime's two-phase `redraw`: the backing screen
//! (Home) renders first at the full area, then the Search screen
//! renders on top at the same area. The new overlay layout must
//! touch only the footer input strip and the centered results
//! block — every other cell stays as Home rendered it.
//!
//! See `plan/2-search.md` §13.

use blockexplorer_tui::{
    adapters::ui::{HomeScreen, Screen, SearchScreen, search_feed},
    application::{ConnectionStatus, HomeViewModel},
    domain::{Chain, ResolvedEntity},
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

use crate::support::stubs::{GasSnapshotFixture, NetworkStatusFixture};

/// Render helper that composites Home + Search into the same
/// buffer, mirroring `src/infra/runtime.rs::redraw`.
fn render_composite(width: u16, height: u16) -> Buffer {
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
    let home = HomeScreen::new(view);

    let (feed, _sender) = search_feed();
    // on_detail is never invoked in this test but must be present.
    let detail_factory: blockexplorer_tui::adapters::ui::DetailFactory =
        Box::new(|_: ResolvedEntity| unreachable!("not triggered by this test"));
    let search = SearchScreen::new(feed, detail_factory);

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| {
            let area = frame.area();
            home.render(frame, area);
            search.render(frame, area);
        })
        .expect("draw");
    terminal.backend().buffer().clone()
}

fn row_text(buffer: &Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol().to_string())
        .collect()
}

fn buffer_contains(buffer: &Buffer, needle: &str) -> bool {
    (0..buffer.area.height).any(|y| row_text(buffer, y).contains(needle))
}

/// Find the first row containing `needle`.
fn row_of(buffer: &Buffer, needle: &str) -> Option<u16> {
    (0..buffer.area.height).find(|y| row_text(buffer, *y).contains(needle))
}

/// Home's header line ("Chain: Ethereum") must still be visible
/// after the Search overlay is painted on top. The header sits
/// inside a 3-row block at the top of the screen, far above the
/// footer input strip and outside the centered results block.
#[test]
fn home_header_stays_visible_behind_search_overlay() {
    let buffer = render_composite(120, 30);

    assert!(
        buffer_contains(&buffer, "Chain: Ethereum"),
        "Home header must survive the overlay; got:\n{}",
        (0..buffer.area.height)
            .map(|y| row_text(&buffer, y))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The search input must sit as a 3-row footer pinned to the
/// bottom of the buffer. The prompt `> _` (with the user's
/// current input between `>` and `_`) is the canonical marker.
#[test]
fn search_input_renders_in_bottom_footer() {
    let buffer = render_composite(120, 30);

    let prompt_row = row_of(&buffer, "> _").expect("search prompt must be rendered");
    assert!(
        prompt_row >= buffer.area.height - 3,
        "search prompt must sit inside the bottom 3 rows (row {prompt_row} of {})",
        buffer.area.height,
    );
}

/// The centered results block must be painted as a floating
/// modal: its `Candidates` border title is the canonical marker
/// and it must not live at the very bottom (the footer) nor at
/// the very top (Home's header).
#[test]
fn results_block_renders_as_centered_modal() {
    let buffer = render_composite(120, 30);

    let candidates_row = row_of(&buffer, "Candidates").expect("Candidates border title");
    assert!(
        candidates_row > 2 && candidates_row < buffer.area.height - 3,
        "Candidates border must live in the middle of the buffer (row {candidates_row})",
    );
}

/// The Home Network card ("Latest block" + formatted block
/// number) lives on the left half of the screen. With a 60%
/// wide centered results block, the left edge of the buffer on
/// the rows above the footer must still contain Home content —
/// otherwise the overlay painted over Home.
#[test]
fn home_network_card_stays_visible_outside_overlay() {
    let buffer = render_composite(120, 30);

    // The Network card title "Network" sits on the left half.
    assert!(
        buffer_contains(&buffer, "Network"),
        "Network card title must stay visible behind the overlay",
    );
    // Latest block number formatted with thousand separator.
    assert!(
        buffer_contains(&buffer, "21,345,678"),
        "Latest block number from Home must stay visible behind the overlay",
    );
}

/// The three left-most columns of every row above the footer
/// must NOT be painted by the overlay. Ratatui fills `Clear`'d
/// rectangles with blank cells styled with the background
/// colour; on a `TestBackend` that shows up as a space symbol.
/// We rely instead on the top-left cells carrying Home's header
/// border character (the `Block::default().borders(Borders::ALL)`
/// Home uses draws `┌` at (0, 0)).
#[test]
fn overlay_does_not_paint_top_left_corner() {
    let buffer = render_composite(120, 30);

    let corner = buffer[(0, 0)].symbol().to_string();
    assert_eq!(
        corner, "┌",
        "Home's top-left border corner must stay intact (got {corner:?})",
    );
}
