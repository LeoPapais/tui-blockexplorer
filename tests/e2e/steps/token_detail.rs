//! Step definitions for the Token Detail feature.
//!
//! See `plan/8-token-detail.md` section 12.3 (Overview MVP) and
//! section 12.4 (price / transfers / chart).

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{Command, Screen, ScreenStack, TokenDetailScreen, TokenTab},
    domain::{
        Address, BlockNumber, Chain, PriceLookup, PricePoint, PriceSeries, PriceWindow,
        TokenMetadata, TokenOverview, TokenPrice, Transaction, TransferAsset, TransferCategory,
        TransferEvent, TransferPage, TxHash, TxStatus, TxType, UnixTimestamp, Wei,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use cucumber::{given, then, when};
use pretty_assertions::assert_eq;

use crate::{
    steps::search::{build_stack, spawn_token_detail, spawn_token_detail_with_full_feeds},
    world::AppWorld,
};

fn current(stack: &ScreenStack) -> &TokenDetailScreen {
    stack
        .top()
        .expect("stack non-empty")
        .as_any()
        .downcast_ref::<TokenDetailScreen>()
        .expect("top of stack must be a TokenDetailScreen")
}

fn current_mut(stack: &mut ScreenStack) -> &mut TokenDetailScreen {
    stack
        .top_mut()
        .expect("stack non-empty")
        .as_any_mut()
        .downcast_mut::<TokenDetailScreen>()
        .expect("top of stack must be a TokenDetailScreen")
}

async fn tick_until<F>(stack: &mut ScreenStack, mut predicate: F) -> bool
where
    F: FnMut(&ScreenStack) -> bool,
{
    for _ in 0..50 {
        let cmd = stack.top_mut().expect("stack non-empty").tick();
        match cmd {
            Command::None | Command::Refresh => {}
            other => panic!("unexpected command: {other:?}"),
        }
        if predicate(stack) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

fn make_key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    }
}

fn parse_window(label: &str) -> PriceWindow {
    match label {
        "1d" => PriceWindow::D1,
        "1m" => PriceWindow::M1,
        "1y" => PriceWindow::Y1,
        other => panic!("unknown window {other}"),
    }
}

// ---------------------------------------------------------------------------
// Givens
// ---------------------------------------------------------------------------

#[given(
    regex = r#"^the token reader knows "(0x[0-9a-fA-F]{40})" as "([^"]+)" / "([^"]+)" decimals (\d+) supply (\d+)$"#
)]
async fn reader_knows_token(
    world: &mut AppWorld,
    addr_hex: String,
    symbol: String,
    name: String,
    decimals: u8,
    supply: u128,
) {
    let address = Address::from_hex(&addr_hex).unwrap();
    world.token_reader_stub.insert(TokenOverview {
        metadata: TokenMetadata {
            address,
            symbol,
            name,
            decimals,
        },
        total_supply: supply,
        price: PriceLookup::Pending,
    });
}

#[given(regex = r#"^the prices stub returns (\d+(?:\.\d+)?) USD for "(0x[0-9a-fA-F]{40})"$"#)]
async fn prices_stub_has_spot(world: &mut AppWorld, value: f64, addr_hex: String) {
    let addr = Address::from_hex(&addr_hex).unwrap();
    world.prices_stub.set_single(
        addr,
        TokenPrice {
            currency: "usd".into(),
            value,
            as_of: UnixTimestamp::from_seconds(1_700_000_000),
        },
    );
}

#[given(regex = r#"^the Prices API returns 404 for "(0x[0-9a-fA-F]{40})"$"#)]
async fn prices_api_returns_404(world: &mut AppWorld, addr_hex: String) {
    let addr = Address::from_hex(&addr_hex).unwrap();
    world.prices_stub.set_unsupported(addr, "alchemy-prices");
}

#[given(
    regex = r#"^the token reader knows "(0x[0-9a-fA-F]{40})" as an incomplete non-ERC20 contract$"#
)]
async fn reader_knows_incomplete_token(world: &mut AppWorld, addr_hex: String) {
    let address = Address::from_hex(&addr_hex).unwrap();
    world.token_reader_stub.insert(TokenOverview {
        metadata: TokenMetadata {
            address,
            symbol: String::new(),
            name: String::new(),
            decimals: 0,
        },
        total_supply: 0,
        price: PriceLookup::Pending,
    });
}

#[given(
    regex = r#"^the prices stub returns (\d+) points for window "([^"]+)" on "(0x[0-9a-fA-F]{40})"$"#
)]
async fn prices_stub_has_history(
    world: &mut AppWorld,
    n: u64,
    window_label: String,
    addr_hex: String,
) {
    let addr = Address::from_hex(&addr_hex).unwrap();
    let window = parse_window(&window_label);
    let points = (0..n)
        .map(|i| PricePoint {
            at: UnixTimestamp::from_seconds(1_700_000_000 + i * 3600),
            value: 1.0 + (i as f64) * 0.01,
        })
        .collect();
    world.prices_stub.set_history(
        addr,
        PriceSeries {
            window,
            currency: "usd".into(),
            points,
        },
    );
}

#[given(
    regex = r#"^the transfers stub returns 1 transfer for token "(0x[0-9a-fA-F]{40})" with hash "(0x[0-9a-fA-F]{64})"$"#
)]
async fn transfers_stub_has_one_for_contract(
    world: &mut AppWorld,
    addr_hex: String,
    hash_hex: String,
) {
    let contract = Address::from_hex(&addr_hex).unwrap();
    let hash = TxHash::from_hex(&hash_hex).unwrap();
    let from = Address::from_hex("0x1111111111111111111111111111111111111111").unwrap();
    let to = Address::from_hex("0x2222222222222222222222222222222222222222").unwrap();
    let page = TransferPage {
        events: vec![TransferEvent {
            chain: Chain::Ethereum,
            block_number: BlockNumber::new(21_345_678),
            tx_hash: hash,
            from,
            to: Some(to),
            asset: TransferAsset::Erc20 {
                contract,
                symbol: "USDC".into(),
                decimals: 6,
            },
            value: Wei::new(1_000_000),
            category: TransferCategory::Erc20,
        }],
        next_cursor: None,
    };
    world.transfers_stub.set_page_for_contract(contract, page);
    world.last_tx_hash = Some(hash);
}

#[given("the tx reader knows that transfer")]
async fn tx_reader_knows_that_transfer(world: &mut AppWorld) {
    let hash = world.last_tx_hash.expect("transfer hash captured");
    let from = Address::from_hex("0x1111111111111111111111111111111111111111").unwrap();
    let to = Address::from_hex("0x2222222222222222222222222222222222222222").unwrap();
    let tx = Transaction {
        chain: Chain::Ethereum,
        hash,
        status: TxStatus::Success,
        block_number: Some(BlockNumber::new(21_345_678)),
        block_hash: None,
        tx_index: Some(0),
        from,
        to: Some(to),
        value: Wei::new(0),
        gas_price: Wei::new(25_000_000_000),
        gas_used: Some(21_000),
        gas_limit: 21_000,
        nonce: 0,
        tx_type: TxType::Legacy,
        input: Vec::new(),
        logs: Vec::new(),
        raw_json: "{}".to_string(),
    };
    world.tx_reader_stub.insert(tx);
}

// ---------------------------------------------------------------------------
// Whens
// ---------------------------------------------------------------------------

#[when(regex = r#"^the user opens TokenDetail for "(0x[0-9a-fA-F]{40})"$"#)]
async fn opens_token_detail(world: &mut AppWorld, addr_hex: String) {
    build_stack(world);
    let addr = Address::from_hex(&addr_hex).unwrap();
    let reader = world.token_reader_stub.clone();
    let screen = spawn_token_detail(Chain::Ethereum, addr, reader);
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

#[when(regex = r#"^the user opens TokenDetail with full feeds for "(0x[0-9a-fA-F]{40})"$"#)]
async fn opens_token_detail_with_full_feeds(world: &mut AppWorld, addr_hex: String) {
    build_stack(world);
    let addr = Address::from_hex(&addr_hex).unwrap();
    let reader = world.token_reader_stub.clone();
    let prices = world.prices_stub.clone();
    let transfers = world.transfers_stub.clone();
    let tx_reader = world.tx_reader_stub.clone();
    let stream = world.price_stream_stub.clone();
    // Wire the `c` shortcut (plan/8 §13.2) to the Contract Detail
    // screen backed by the address-reader + proxy-detector stubs
    // already primed by earlier Givens.
    let address_reader = world.address_reader_stub.clone();
    let proxy_detector = world.proxy_detector_stub.clone();
    let open_contract: blockexplorer_tui::adapters::ui::TokenOpenContractFactory =
        Box::new(move |addr| {
            crate::steps::search::spawn_contract_detail(
                Chain::Ethereum,
                addr,
                address_reader.clone(),
                proxy_detector.clone(),
            )
        });
    let screen = spawn_token_detail_with_full_feeds(
        Chain::Ethereum,
        addr,
        reader,
        prices,
        transfers,
        tx_reader,
        Some(stream),
        Some(open_contract),
    );
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

#[when(regex = r#"^the user presses "([123])" to select window "([^"]+)"$"#)]
async fn presses_digit_to_select_window(world: &mut AppWorld, digit: char, _window: String) {
    let stack = world.stack.as_mut().expect("stack");
    let screen = current_mut(stack);
    let cmd = screen.handle_key(make_key(KeyCode::Char(digit)));
    assert!(
        matches!(cmd, Command::None),
        "digit key must not emit a nav command"
    );
}

#[when(regex = r#"^the user presses "c" to view as contract$"#)]
async fn presses_c_to_view_as_contract(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    // Make sure the overview reached the incomplete state before
    // dispatching the key — otherwise `c` is inert (see
    // plan/8-token-detail.md §13.2).
    tick_until(stack, |s| current(s).is_incomplete_badge_active()).await;
    let cmd = current_mut(stack).handle_key(make_key(KeyCode::Char('c')));
    match cmd {
        Command::Push(screen) => stack.push(screen),
        other => panic!("expected Push from `c`, got {other:?}"),
    }
}

#[given(regex = r#"^the price stream pushes (\d+(?:\.\d+)?) USD for "(0x[0-9a-fA-F]{40})"$"#)]
async fn price_stream_pushes_given(world: &mut AppWorld, value: f64, addr_hex: String) {
    push_stream_value(world, value, &addr_hex).await;
}

#[when(regex = r#"^the price stream pushes (\d+(?:\.\d+)?) USD for "(0x[0-9a-fA-F]{40})"$"#)]
async fn price_stream_pushes_when(world: &mut AppWorld, value: f64, addr_hex: String) {
    push_stream_value(world, value, &addr_hex).await;
}

#[then(regex = r#"^the price stream pushes (\d+(?:\.\d+)?) USD for "(0x[0-9a-fA-F]{40})"$"#)]
async fn price_stream_pushes_then(world: &mut AppWorld, value: f64, addr_hex: String) {
    push_stream_value(world, value, &addr_hex).await;
}

async fn push_stream_value(world: &mut AppWorld, value: f64, addr_hex: &str) {
    // Wait for the async feed task to have subscribed before pushing.
    // Without this, the stub broadcasts land in the void because the
    // feed has not yet wired its receiver.
    for _ in 0..50 {
        if world
            .price_stream_stub
            .clone()
            .subscriber_count(Address::from_hex(addr_hex).unwrap())
            > 0
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let addr = Address::from_hex(addr_hex).unwrap();
    world.price_stream_stub.push(
        addr,
        TokenPrice {
            currency: "usd".into(),
            value,
            as_of: UnixTimestamp::from_seconds(1_700_000_000),
        },
    );
    // Give the dispatcher one extra tick to forward the push onto
    // `price_tx` before the next Then asserts.
    tokio::time::sleep(Duration::from_millis(10)).await;
}

#[when("the user switches to the Transfers tab")]
async fn switches_to_transfers(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    // Wait for transfers to arrive so Enter has something to act on.
    tick_until(stack, |s| current(s).transfers().is_some()).await;
    let screen = current_mut(stack);
    // Cycle through tabs with Tab until we land on Transfers.
    for _ in 0..TokenTab::ALL_COUNT {
        if screen.active_tab() == TokenTab::Transfers {
            break;
        }
        let _ = screen.handle_key(make_key(KeyCode::Tab));
    }
    assert_eq!(screen.active_tab(), TokenTab::Transfers);
}

#[when("the user presses Enter on the first transfer row")]
async fn presses_enter_on_first_transfer(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    // Make sure the selection is on the first row.
    tick_until(stack, |s| {
        current(s)
            .transfers()
            .map(|p| !p.events.is_empty())
            .unwrap_or(false)
    })
    .await;
    let cmd = current_mut(stack).handle_key(make_key(KeyCode::Enter));
    match cmd {
        Command::Push(screen) => stack.push(screen),
        other => panic!("expected Push, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Thens
// ---------------------------------------------------------------------------

#[then(
    regex = r#"^once the token is loaded, the Overview shows symbol "([^"]+)" and supply (\d+)$"#
)]
async fn overview_shows_symbol_and_supply(
    world: &mut AppWorld,
    expected_symbol: String,
    expected_supply: u128,
) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).current().is_some()).await;
    let ov = current(stack).current().expect("loaded");
    assert_eq!(ov.metadata.symbol, expected_symbol);
    assert_eq!(ov.total_supply, expected_supply);
}

#[then("the Overview stays in the loading state")]
async fn overview_stays_loading(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    let loaded = tick_until(stack, |s| current(s).current().is_some()).await;
    assert!(!loaded, "token was unexpectedly loaded");
    assert!(current(stack).current().is_none());
}

#[then("once the feeds complete, the Overview shows the incomplete-token badge")]
async fn overview_shows_incomplete_badge(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    let flagged = tick_until(stack, |s| current(s).is_incomplete_badge_active()).await;
    assert!(
        flagged,
        "Overview never reached the incomplete-token badge state"
    );
}

#[then(regex = r#"^once the feeds complete, the Overview price is "\$([0-9.]+)"$"#)]
async fn overview_price_renders(world: &mut AppWorld, expected: f64) {
    let stack = world.stack.as_mut().expect("stack");
    // Tick until the displayed price actually converges to `expected`
    // (not merely Some) so the live-stream scenarios that push two
    // values in a row can assert on each one.
    tick_until(stack, |s| {
        current(s)
            .price()
            .map(|p| (p.value - expected).abs() < 1e-6)
            .unwrap_or(false)
    })
    .await;
    let price = current(stack).price().expect("loaded");
    let diff = (price.value - expected).abs();
    assert!(
        diff < 1e-6,
        "price {} differs from expected {}",
        price.value,
        expected,
    );
}

#[then(
    regex = r#"^once the feeds complete, the Overview row for price renders "\(not indexed by ([^)]+)\)"$"#
)]
async fn overview_price_renders_unsupported(world: &mut AppWorld, provider: String) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        matches!(current(s).price_lookup(), PriceLookup::Unsupported { .. },)
    })
    .await;
    match current(stack).price_lookup() {
        PriceLookup::Unsupported { provider: got } => assert_eq!(*got, provider),
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[then(regex = r#"^the active window is "([^"]+)"$"#)]
async fn active_window_is(world: &mut AppWorld, label: String) {
    let stack = world.stack.as_ref().expect("stack");
    assert_eq!(current(stack).active_window(), parse_window(&label));
}

#[then(regex = r#"^once the feeds complete, the chart holds (\d+) points for window "([^"]+)"$"#)]
async fn chart_holds_points(world: &mut AppWorld, n: usize, label: String) {
    let expected_window = parse_window(&label);
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current(s)
            .active_series()
            .map(|series| series.window == expected_window && series.points.len() == n)
            .unwrap_or(false)
    })
    .await;
    let series = current(stack).active_series().expect("series present");
    assert_eq!(series.window, expected_window);
    assert_eq!(series.points.len(), n);
}
