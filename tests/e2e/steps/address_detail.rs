//! Step definitions for the Address Detail feature.
//!
//! See `plan/6-address-detail.md` section 12.3. Scenarios live in
//! `tests/e2e/features/address_detail.feature`.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{AddressDetailScreen, AddressTab, Command, ScreenStack},
    domain::{
        Address, AddressKind, AddressOverview, BlockNumber, Chain, TokenHolding, TokenMetadata,
        TransferAsset, TransferCategory, TransferEvent, TransferPage, TxHash, Wei,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use cucumber::{given, then, when};

use crate::{
    steps::search::{
        build_stack, spawn_address_detail, spawn_address_detail_with_full_feeds,
        spawn_address_detail_with_transfers,
    },
    world::AppWorld,
};

fn current(stack: &ScreenStack) -> &AddressDetailScreen {
    stack
        .top()
        .expect("stack non-empty")
        .as_any()
        .downcast_ref::<AddressDetailScreen>()
        .expect("top of stack must be an AddressDetailScreen")
}

async fn tick_until<F>(stack: &mut ScreenStack, mut predicate: F)
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
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn insert_overview(world: &AppWorld, addr_hex: &str, kind: AddressKind, balance: u128, nonce: u64) {
    let delegated_to = match kind {
        AddressKind::Eoa { delegated_to } => delegated_to,
        AddressKind::Contract => None,
    };
    world.address_reader_stub.insert(AddressOverview {
        chain: Chain::Ethereum,
        address: Address::from_hex(addr_hex).unwrap(),
        balance: Wei::new(balance),
        nonce,
        kind,
        delegated_to,
        ens_name: None,
    });
}

#[given(
    regex = r#"^the address reader knows EOA "(0x[0-9a-fA-F]{40})" with balance (\d+) and nonce (\d+)$"#
)]
async fn reader_knows_eoa(world: &mut AppWorld, addr_hex: String, balance: u128, nonce: u64) {
    insert_overview(
        world,
        &addr_hex,
        AddressKind::Eoa { delegated_to: None },
        balance,
        nonce,
    );
}

#[given(
    regex = r#"^the address reader knows contract "(0x[0-9a-fA-F]{40})" with balance (\d+) and nonce (\d+)$"#
)]
async fn reader_knows_contract(world: &mut AppWorld, addr_hex: String, balance: u128, nonce: u64) {
    insert_overview(world, &addr_hex, AddressKind::Contract, balance, nonce);
}

#[when(regex = r#"^the user opens AddressDetail for "(0x[0-9a-fA-F]{40})"$"#)]
async fn opens_address_detail(world: &mut AppWorld, addr_hex: String) {
    build_stack(world);
    let addr = Address::from_hex(&addr_hex).unwrap();
    let reader = world.address_reader_stub.clone();
    let screen = spawn_address_detail(Chain::Ethereum, addr, reader);
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

#[then(regex = r#"^once the address is loaded, the Overview shows kind "([^"]+)"$"#)]
async fn overview_shows_kind(world: &mut AppWorld, expected: String) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).current().is_some()).await;
    let ov = current(stack).current().expect("loaded");
    let label = match ov.kind {
        AddressKind::Eoa {
            delegated_to: Some(_),
        } => "EOA (7702 delegated)",
        AddressKind::Eoa { delegated_to: None } => "EOA",
        AddressKind::Contract => "Contract",
    };
    assert_eq!(label, expected);
}

#[then(regex = r#"^the Overview shows balance (\d+)$"#)]
async fn overview_shows_balance(world: &mut AppWorld, expected: u128) {
    let ov = current(world.stack.as_ref().expect("stack"))
        .current()
        .expect("loaded");
    assert_eq!(ov.balance.value(), expected);
}

// ---------------------------------------------------------------------------
// Transactions tab (plan 6 section 12.4.1)
// ---------------------------------------------------------------------------

use blockexplorer_tui::domain::{BlockHash, Transaction, TxStatus, TxType};

fn seed_tx_reader(world: &AppWorld, hash: TxHash) {
    world.tx_reader_stub.insert(Transaction {
        chain: Chain::Ethereum,
        hash,
        status: TxStatus::Success,
        block_number: Some(BlockNumber::new(21_000_000)),
        block_hash: Some(
            BlockHash::from_hex(
                "0xaaaa000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
        ),
        tx_index: Some(0),
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(Address::from_hex("0x0000000000000000000000000000000000000099").unwrap()),
        value: Wei::new(1_000_000_000_000_000_000),
        gas_price: Wei::new(14_000_000_000),
        gas_used: Some(21_000),
        gas_limit: 21_000,
        nonce: 0,
        tx_type: TxType::Legacy,
        input: Vec::new(),
        logs: Vec::new(),
        raw_json: "{}".to_string(),
    });
}

fn sample_transfer(block: u64, hash_hex: &str, from: &str, to: &str) -> TransferEvent {
    TransferEvent {
        chain: Chain::Ethereum,
        block_number: BlockNumber::new(block),
        tx_hash: TxHash::from_hex(hash_hex).unwrap(),
        from: Address::from_hex(from).unwrap(),
        to: Some(Address::from_hex(to).unwrap()),
        asset: TransferAsset::Native {
            symbol: "ETH".into(),
        },
        value: Wei::new(1_000_000_000_000_000_000),
        category: TransferCategory::External,
    }
}

#[given(regex = r#"^the transfers feed knows (\d+) events for "(0x[0-9a-fA-F]{40})"$"#)]
async fn transfers_feed_knows_n(world: &mut AppWorld, count: u32, addr_hex: String) {
    let address = Address::from_hex(&addr_hex).unwrap();
    let hashes = [
        "0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa",
        "0xbbbb016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394bb",
        "0xcccc016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394cc",
    ];
    let events: Vec<TransferEvent> = (0..count as usize)
        .map(|i| {
            sample_transfer(
                21_000_000 - i as u64,
                hashes[i % hashes.len()],
                &addr_hex,
                "0x0000000000000000000000000000000000000099",
            )
        })
        .collect();
    // Seed the tx reader so Enter on any transfer actually loads a
    // TxDetail.
    for event in &events {
        seed_tx_reader(world, event.tx_hash);
    }
    world.transfers_stub.set_page(
        address,
        TransferPage {
            events,
            next_cursor: None,
        },
    );
    world.last_address = Some(address);
}

#[when(regex = r#"^the user opens AddressDetail with transfers for "(0x[0-9a-fA-F]{40})"$"#)]
async fn opens_address_detail_with_transfers(world: &mut AppWorld, addr_hex: String) {
    build_stack(world);
    let addr = Address::from_hex(&addr_hex).unwrap();
    let reader = world.address_reader_stub.clone();
    let transfers = world.transfers_stub.clone();
    let tx_reader = world.tx_reader_stub.clone();
    let screen =
        spawn_address_detail_with_transfers(Chain::Ethereum, addr, reader, transfers, tx_reader);
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

// "the user switches to the Transactions tab" is defined in
// block_detail.rs and simply sends a Tab key press to whatever screen
// is on top, so it already drives the AddressDetailScreen correctly.

#[when("the user switches to the Tokens tab")]
async fn switches_to_tokens_tab(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    // Tab bar cycle is Overview -> Transactions -> Tokens, so two
    // presses land on Tokens regardless of the starting tab as long
    // as we begin on Overview (the default).
    press_key(stack, KeyCode::Tab);
    press_key(stack, KeyCode::Tab);
}

#[given(regex = r#"^the portfolio feed knows (\d+) holdings for "(0x[0-9a-fA-F]{40})"$"#)]
async fn portfolio_feed_knows_n(world: &mut AppWorld, count: u32, addr_hex: String) {
    let address = Address::from_hex(&addr_hex).unwrap();
    let holdings: Vec<TokenHolding> = (0..count as usize)
        .map(|i| TokenHolding {
            metadata: TokenMetadata {
                address: Address::from_hex(&format!("0x{:040x}", (0x1000_0000_u64 + i as u64)))
                    .unwrap(),
                symbol: format!("TKN{i}"),
                name: format!("Token {i}"),
                decimals: 18,
            },
            balance: Wei::new(1_000 * (i as u128 + 1)),
            price: blockexplorer_tui::domain::PriceLookup::Pending,
        })
        .collect();
    // Ensure the token reader can resolve every contract so Enter on
    // a holding can open a TokenDetail screen.
    for h in &holdings {
        seed_token_reader(world, &h.metadata);
    }
    world.portfolio_stub.set_holdings(address, holdings);
    world.last_address = Some(address);
}

fn seed_token_reader(world: &AppWorld, metadata: &TokenMetadata) {
    use blockexplorer_tui::domain::{PriceLookup, TokenOverview};
    world.token_reader_stub.insert(TokenOverview {
        metadata: metadata.clone(),
        total_supply: 0,
        price: PriceLookup::Pending,
    });
}

#[when(regex = r#"^the user opens AddressDetail with full feeds for "(0x[0-9a-fA-F]{40})"$"#)]
async fn opens_address_detail_with_full_feeds(world: &mut AppWorld, addr_hex: String) {
    build_stack(world);
    let addr = Address::from_hex(&addr_hex).unwrap();
    let reader = world.address_reader_stub.clone();
    let transfers = world.transfers_stub.clone();
    let portfolio = world.portfolio_stub.clone();
    let tx_reader = world.tx_reader_stub.clone();
    let token_reader = world.token_reader_stub.clone();
    let screen = spawn_address_detail_with_full_feeds(
        Chain::Ethereum,
        addr,
        reader,
        transfers,
        portfolio,
        tx_reader,
        token_reader,
    );
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

#[then(regex = r#"^once loaded, the Tokens tab lists (\d+) holdings$"#)]
async fn tokens_tab_lists_n(world: &mut AppWorld, expected: u32) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).holdings().is_some()).await;
    let screen = current(stack);
    assert_eq!(screen.active_tab(), AddressTab::Tokens);
    let count = screen.holdings().map(|h| h.len()).unwrap_or(0);
    assert_eq!(count, expected as usize);
}

#[then("once loaded, the Tokens tab reports no holdings")]
async fn tokens_tab_empty(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).holdings().is_some()).await;
    let screen = current(stack);
    assert!(screen.holdings().map(|h| h.is_empty()).unwrap_or(false));
}

#[when("the user selects the first holding and presses Enter")]
async fn selects_first_holding_and_enter(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    press_key(stack, KeyCode::Home);
    press_key(stack, KeyCode::Enter);
}

#[then("once loaded, the tab bar includes the Contract tab")]
async fn tab_bar_has_contract(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).current().is_some()).await;
    let tabs = current(stack).tabs();
    assert!(
        tabs.contains(&AddressTab::Contract),
        "Contract tab missing from {tabs:?}"
    );
}

#[then("once loaded, the tab bar does not include the Contract tab")]
async fn tab_bar_no_contract(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).current().is_some()).await;
    let tabs = current(stack).tabs();
    assert!(
        !tabs.contains(&AddressTab::Contract),
        "Contract tab unexpectedly present in {tabs:?}"
    );
}

#[then(regex = r#"^once loaded, the Transactions tab lists (\d+) transfers$"#)]
async fn transactions_tab_lists_n(world: &mut AppWorld, expected: u32) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current(s)
            .transfers()
            .map(|p| !p.events.is_empty())
            .unwrap_or(false)
    })
    .await;
    let screen = current(stack);
    assert_eq!(screen.active_tab(), AddressTab::Transactions);
    let count = screen.transfers().map(|p| p.events.len()).unwrap_or(0);
    assert_eq!(count, expected as usize);
}

#[when("the user selects the first transfer and presses Enter")]
async fn selects_first_and_enter(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    press_key(stack, KeyCode::Home);
    press_key(stack, KeyCode::Enter);
}

fn press_key(stack: &mut ScreenStack, code: KeyCode) {
    let screen = stack.top_mut().expect("stack non-empty");
    let cmd = screen.handle_key(KeyEvent {
        code,
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    });
    stack.apply_command(cmd);
}

// ---------------------------------------------------------------------------
// ERC-20 inline Token tab steps
// ---------------------------------------------------------------------------

#[when(regex = r#"^the user opens AddressDetail with ERC-20 probe for "(0x[0-9a-fA-F]{40})"$"#)]
async fn opens_address_detail_with_erc20_probe(world: &mut AppWorld, addr_hex: String) {
    use crate::steps::search::spawn_address_detail_with_erc20_probe;
    build_stack(world);
    let addr = Address::from_hex(&addr_hex).unwrap();
    let reader = world.address_reader_stub.clone();
    let transfers = world.transfers_stub.clone();
    let portfolio = world.portfolio_stub.clone();
    let tx_reader = world.tx_reader_stub.clone();
    let token_reader = world.token_reader_stub.clone();
    let prices = world.prices_stub.clone();
    let screen = spawn_address_detail_with_erc20_probe(
        Chain::Ethereum,
        addr,
        reader,
        transfers,
        portfolio,
        tx_reader,
        token_reader,
        prices,
    );
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

#[then("once the ERC-20 probe completes, the tab bar includes the Token tab")]
async fn tab_bar_has_token(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).tabs().contains(&AddressTab::Token)).await;
    let tabs = current(stack).tabs();
    assert!(
        tabs.contains(&AddressTab::Token),
        "Token tab missing from {tabs:?}",
    );
}

#[then("once loaded, the tab bar does not include the Token tab")]
async fn tab_bar_no_token(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    // Wait for the overview to arrive so the probe has a chance to
    // run (or be skipped). The Token tab may only be added later so
    // we also sleep a short window to give the probe time to either
    // return None or never fire at all.
    tick_until(stack, |s| current(s).current().is_some()).await;
    tokio::time::sleep(Duration::from_millis(40)).await;
    let cmd = stack.top_mut().unwrap().tick();
    assert!(matches!(cmd, Command::None | Command::Refresh));
    let tabs = current(stack).tabs();
    assert!(
        !tabs.contains(&AddressTab::Token),
        "Token tab unexpectedly present in {tabs:?}",
    );
}

// ---------------------------------------------------------------------------
// Reverse ENS + Y / e clipboard bindings (plan/6 §11 "Shipped")
// ---------------------------------------------------------------------------

#[given(
    regex = r#"^the ENS resolver knows that "(0x[0-9a-fA-F]{40})" resolves reverse to "([^"]+)"$"#
)]
async fn ens_knows_reverse(world: &mut AppWorld, addr_hex: String, name: String) {
    let address = Address::from_hex(&addr_hex).unwrap();
    world.ens_stub.set_reverse(address, &name);
}

#[when(regex = r#"^the user opens AddressDetail with reverse ENS for "(0x[0-9a-fA-F]{40})"$"#)]
async fn opens_address_detail_with_reverse_ens(world: &mut AppWorld, addr_hex: String) {
    use crate::steps::search::spawn_address_detail_with_reverse_ens;
    build_stack(world);
    let addr = Address::from_hex(&addr_hex).unwrap();
    let reader = world.address_reader_stub.clone();
    let ens = world.ens_stub.clone();
    let screen = spawn_address_detail_with_reverse_ens(Chain::Ethereum, addr, reader, ens);
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

#[then(regex = r#"^once the overview is loaded, pressing Y copies "([^"]+)"$"#)]
async fn overview_loaded_then_y_copies(world: &mut AppWorld, expected: String) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).current().is_some()).await;
    let screen = stack.top_mut().expect("stack non-empty");
    let cmd = screen.handle_key(KeyEvent {
        code: KeyCode::Char('Y'),
        modifiers: KeyModifiers::SHIFT,
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    });
    match cmd {
        Command::None | Command::Refresh => {}
        other => panic!("unexpected command after Y: {other:?}"),
    }
    let copied = current(stack).last_copied_value().map(str::to_string);
    assert_eq!(copied.as_deref(), Some(expected.as_str()));
}

#[when("the user presses e on the Tokens tab")]
async fn presses_e_on_tokens(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    assert_eq!(current(stack).active_tab(), AddressTab::Tokens);
    press_key(stack, KeyCode::Char('e'));
}

#[given(regex = r#"^the portfolio feed knows priced holdings for "(0x[0-9a-fA-F]{40})"$"#)]
async fn portfolio_feed_knows_priced(world: &mut AppWorld, addr_hex: String) {
    use blockexplorer_tui::domain::{PriceLookup, TokenPrice, UnixTimestamp};
    let address = Address::from_hex(&addr_hex).unwrap();
    // Two priced holdings summing to $2.00 + one unsupported ⇒
    // header must read "$2.00  (2 priced, 1 not priced)".
    let holdings = vec![
        TokenHolding {
            metadata: TokenMetadata {
                address: Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
                symbol: "USDC".into(),
                name: "USD Coin".into(),
                decimals: 6,
            },
            balance: Wei::new(1_000_000),
            price: PriceLookup::Available(TokenPrice {
                currency: "usd".into(),
                value: 1.0,
                as_of: UnixTimestamp::from_seconds(1),
            }),
        },
        TokenHolding {
            metadata: TokenMetadata {
                address: Address::from_hex("0x0000000000000000000000000000000000000002").unwrap(),
                symbol: "ABC".into(),
                name: "Another".into(),
                decimals: 18,
            },
            balance: Wei::new(1_000_000_000_000_000_000),
            price: PriceLookup::Available(TokenPrice {
                currency: "usd".into(),
                value: 1.0,
                as_of: UnixTimestamp::from_seconds(1),
            }),
        },
        TokenHolding {
            metadata: TokenMetadata {
                address: Address::from_hex("0xe6a537a407488807f0bbeb0038b79004f19dddfb").unwrap(),
                symbol: "BRLA".into(),
                name: "BRLA Token".into(),
                decimals: 18,
            },
            balance: Wei::new(5_000_000_000_000_000_000),
            price: PriceLookup::Unsupported {
                provider: "alchemy-prices",
            },
        },
    ];
    for h in &holdings {
        seed_token_reader(world, &h.metadata);
    }
    world.portfolio_stub.set_holdings(address, holdings);
    world.last_address = Some(address);
}

#[then(
    regex = r#"^once loaded, the Tokens tab shows a USD total of "\$([0-9.]+)" and (\d+) token[s]? not priced$"#
)]
async fn tokens_tab_shows_usd_total(
    world: &mut AppWorld,
    expected_total: f64,
    expected_not: usize,
) {
    use blockexplorer_tui::adapters::ui::address_detail::portfolio_summary;
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).holdings().is_some()).await;
    let screen = current(stack);
    let holdings = screen.holdings().expect("holdings loaded");
    let summary = portfolio_summary(holdings);
    assert!(
        (summary.total_usd - expected_total).abs() < 1e-6,
        "total_usd {} differs from expected {}",
        summary.total_usd,
        expected_total,
    );
    assert_eq!(summary.not_priced, expected_not);
}

#[then(regex = r#"^the Tokens tab renders at least (\d+) distribution chart rows$"#)]
async fn tokens_tab_renders_chart(world: &mut AppWorld, expected_rows: usize) {
    use blockexplorer_tui::adapters::ui::address_detail::{
        portfolio_summary, render_top_distribution,
    };
    let stack = world.stack.as_ref().expect("stack");
    let holdings = current(stack).holdings().expect("holdings loaded");
    let summary = portfolio_summary(holdings);
    let rendered = render_top_distribution(&summary, 20);
    let row_count = rendered.lines().count();
    assert!(
        row_count >= expected_rows,
        "expected at least {expected_rows} rows, got {row_count}:\n{rendered}",
    );
}

#[then(regex = r#"^the clipboard sink holds a Tokens CSV with (\d+) data rows$"#)]
async fn clipboard_has_tokens_csv(world: &mut AppWorld, expected_rows: usize) {
    let stack = world.stack.as_ref().expect("stack");
    let screen = current(stack);
    let csv = screen
        .last_copied_value()
        .expect("clipboard sink must hold a CSV blob");
    let mut lines = csv.lines();
    let header = lines.next().expect("CSV header line");
    assert!(
        header.starts_with("symbol,name,contract,"),
        "unexpected header: {header}",
    );
    let data_rows = lines.filter(|l| !l.is_empty()).count();
    assert_eq!(data_rows, expected_rows);
}

#[then(regex = r#"^the inline Token overview shows symbol "([^"]+)" and price "\$([0-9.]+)"$"#)]
async fn inline_token_shows_symbol_and_price(
    world: &mut AppWorld,
    expected_symbol: String,
    expected_price: f64,
) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current(s).token_overview().is_some() && current(s).token_price().is_some()
    })
    .await;
    let screen = current(stack);
    let ov = screen.token_overview().expect("loaded");
    assert_eq!(ov.metadata.symbol, expected_symbol);
    let price = screen.token_price().expect("price loaded");
    assert!(
        (price.value - expected_price).abs() < 1e-6,
        "price {} differs from expected {}",
        price.value,
        expected_price,
    );
}
