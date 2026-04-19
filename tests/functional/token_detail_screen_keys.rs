//! Key + rendering tests for [`TokenDetailScreen`] covering the
//! follow-ups tracked in `plan/15-backlog.md` §8.9:
//!
//! - incomplete-metadata empty state and the `c` → `View as Contract`
//!   shortcut (plan/8 §13.2),
//! - live price streaming through `price_rx` appending a point to the
//!   active-window series (plan/8 §13.1).

use std::any::Any;

use blockexplorer_tui::adapters::ui::{
    Command, Screen, TokenDetailScreen, TokenFeed, TokenFeedSender, token_feed,
    token_detail::OpenContractFactory,
};
use blockexplorer_tui::domain::{
    Address, Chain, PriceLookup, PricePoint, PriceSeries, PriceWindow, TokenMetadata,
    TokenOverview, TokenPrice, UnixTimestamp,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn addr() -> Address {
    Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
}

fn feeds() -> (TokenFeed, TokenFeedSender) {
    token_feed()
}

fn standard_overview() -> TokenOverview {
    TokenOverview {
        metadata: TokenMetadata {
            address: addr(),
            symbol: "USDC".into(),
            name: "USD Coin".into(),
            decimals: 6,
        },
        total_supply: 35_000_000_000_000,
        price: PriceLookup::Pending,
    }
}

fn incomplete_overview() -> TokenOverview {
    TokenOverview {
        metadata: TokenMetadata {
            address: addr(),
            symbol: String::new(),
            name: String::new(),
            decimals: 0,
        },
        total_supply: 0,
        price: PriceLookup::Pending,
    }
}

// ---------------------------------------------------------------------------
// Empty state
// ---------------------------------------------------------------------------

/// Minimal stand-in for the Contract Detail screen. Tests only need to
/// assert a push happened and downcast the pushed screen, not run the
/// real contract loader.
struct FakeContractScreen {
    address: Address,
}

impl Screen for FakeContractScreen {
    fn title(&self) -> &str {
        "Contract"
    }
    fn render(&self, _frame: &mut ratatui::Frame<'_>, _area: ratatui::layout::Rect) {}
    fn handle_key(&mut self, _key: KeyEvent) -> Command {
        Command::None
    }
    fn tick(&mut self) -> Command {
        Command::None
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[test]
fn is_incomplete_badge_is_inactive_by_default() {
    let (feed, sender) = feeds();
    let mut screen = TokenDetailScreen::loading(Chain::Ethereum, addr(), feed);
    sender.updates_tx.send(standard_overview()).unwrap();
    let _ = screen.tick();
    assert!(!screen.is_incomplete_badge_active());
}

#[test]
fn is_incomplete_badge_turns_on_when_metadata_is_degenerate() {
    let (feed, sender) = feeds();
    let mut screen = TokenDetailScreen::loading(Chain::Ethereum, addr(), feed);
    sender.updates_tx.send(incomplete_overview()).unwrap();
    let _ = screen.tick();
    assert!(screen.is_incomplete_badge_active());
}

#[test]
fn c_key_pushes_contract_screen_when_badge_is_active() {
    let (feed, sender) = feeds();
    let open_contract: OpenContractFactory = Box::new(|a| Box::new(FakeContractScreen { address: a }));
    let mut screen = TokenDetailScreen::with_factories(
        Chain::Ethereum,
        addr(),
        feed,
        None,
        Some(open_contract),
    );
    sender.updates_tx.send(incomplete_overview()).unwrap();
    let _ = screen.tick();

    let cmd = screen.handle_key(key(KeyCode::Char('c')));
    match cmd {
        Command::Push(pushed) => {
            let fake = pushed
                .as_any()
                .downcast_ref::<FakeContractScreen>()
                .expect("pushed screen must be FakeContractScreen");
            assert_eq!(fake.address, addr());
        }
        other => panic!("expected Command::Push, got {other:?}"),
    }
}

#[test]
fn c_key_is_inert_when_badge_is_inactive() {
    let (feed, sender) = feeds();
    let open_contract: OpenContractFactory = Box::new(|a| Box::new(FakeContractScreen { address: a }));
    let mut screen = TokenDetailScreen::with_factories(
        Chain::Ethereum,
        addr(),
        feed,
        None,
        Some(open_contract),
    );
    sender.updates_tx.send(standard_overview()).unwrap();
    let _ = screen.tick();

    let cmd = screen.handle_key(key(KeyCode::Char('c')));
    assert!(matches!(cmd, Command::None));
}

#[test]
fn incomplete_body_renders_the_not_erc20_copy() {
    let (feed, sender) = feeds();
    let mut screen = TokenDetailScreen::loading(Chain::Ethereum, addr(), feed);
    sender.updates_tx.send(incomplete_overview()).unwrap();
    let _ = screen.tick();

    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| screen.render(f, f.area())).unwrap();
    let dump = dump_buffer(term.backend().buffer());
    assert!(
        dump.contains("This address does not look like a standard ERC-20"),
        "missing empty-state copy:\n{dump}"
    );
    assert!(
        dump.contains("[c] View as Contract"),
        "missing View-as-Contract shortcut:\n{dump}"
    );
}

fn dump_buffer(buf: &ratatui::buffer::Buffer) -> String {
    let area = buf.area;
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// Live price stream
// ---------------------------------------------------------------------------

fn sample_price(value: f64) -> TokenPrice {
    TokenPrice {
        currency: "usd".into(),
        value,
        as_of: UnixTimestamp::from_seconds(1_700_000_000),
    }
}

#[test]
fn streamed_available_samples_append_to_the_active_window_series() {
    let (feed, sender) = feeds();
    let mut screen = TokenDetailScreen::loading(Chain::Ethereum, addr(), feed);
    sender.updates_tx.send(standard_overview()).unwrap();
    // Seed an empty D1 series so we can watch the live tail grow.
    sender
        .history_tx
        .send(PriceSeries::empty(PriceWindow::D1))
        .unwrap();
    let _ = screen.tick();
    assert_eq!(screen.live_samples_count(), 0);

    sender
        .price_tx
        .send(PriceLookup::Available(sample_price(1.0001)))
        .unwrap();
    let _ = screen.tick();
    assert_eq!(screen.live_samples_count(), 1);
    let series = screen.active_series().expect("series present");
    assert_eq!(series.points.len(), 1);

    sender
        .price_tx
        .send(PriceLookup::Available(sample_price(1.25)))
        .unwrap();
    let _ = screen.tick();
    assert_eq!(screen.live_samples_count(), 2);
    let series = screen.active_series().expect("series present");
    assert_eq!(series.points.len(), 2);
    // Latest push wins the Overview cell.
    let price = screen.price().expect("available");
    assert!((price.value - 1.25).abs() < 1e-9);
}

#[test]
fn streamed_unsupported_refreshes_overview_without_touching_the_chart() {
    let (feed, sender) = feeds();
    let mut screen = TokenDetailScreen::loading(Chain::Ethereum, addr(), feed);
    sender.updates_tx.send(standard_overview()).unwrap();
    sender
        .history_tx
        .send(PriceSeries::empty(PriceWindow::D1))
        .unwrap();
    let _ = screen.tick();

    sender
        .price_tx
        .send(PriceLookup::Unsupported {
            provider: "alchemy-prices",
        })
        .unwrap();
    let _ = screen.tick();

    assert_eq!(screen.live_samples_count(), 0);
    assert!(matches!(
        screen.price_lookup(),
        PriceLookup::Unsupported { provider } if *provider == "alchemy-prices"
    ));
    assert_eq!(screen.active_series().map(|s| s.points.len()), Some(0));
}

#[test]
fn live_tail_is_capped_at_rolling_cap() {
    let (feed, sender) = feeds();
    let mut screen = TokenDetailScreen::loading(Chain::Ethereum, addr(), feed);
    sender.updates_tx.send(standard_overview()).unwrap();
    // Pre-seed a series with ROLLING_CAP - 1 points so the next two
    // streaming samples push us across the cap.
    let mut seed = PriceSeries::empty(PriceWindow::D1);
    seed.points = (0..PriceSeries::ROLLING_CAP - 1)
        .map(|i| PricePoint {
            at: UnixTimestamp::from_seconds(i as u64),
            value: i as f64,
        })
        .collect();
    sender.history_tx.send(seed).unwrap();
    let _ = screen.tick();

    for _ in 0..5 {
        sender
            .price_tx
            .send(PriceLookup::Available(sample_price(2.0)))
            .unwrap();
    }
    let _ = screen.tick();

    let series = screen.active_series().expect("series present");
    assert_eq!(series.points.len(), PriceSeries::ROLLING_CAP);
}
