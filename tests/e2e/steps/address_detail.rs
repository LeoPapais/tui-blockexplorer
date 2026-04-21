//! Step definitions for the Address Detail feature.
//!
//! See `plan/6-address-detail.md` section 12.3. Scenarios live in
//! `tests/e2e/features/address_detail.feature`.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{AddressDetailScreen, AddressTab, Command, ScreenStack},
    domain::{
        AccountTx, AccountTxPage, Address, AddressKind, AddressOverview, BlockNumber, Chain,
        PriceLookup, PricePoint, PriceSeries, PriceWindow, TokenHolding, TokenMetadata,
        TokenOverview, TokenPrice, TransferAsset, TransferCategory, TransferEvent, TransferPage,
        TxHash, UnixTimestamp, Wei,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use cucumber::{given, then, when};
use pretty_assertions::assert_eq;

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

fn sample_account_tx(block: u64, hash_hex: &str, addr_hex: &str) -> AccountTx {
    AccountTx {
        chain: Chain::Ethereum,
        block_number: BlockNumber::new(block),
        tx_hash: TxHash::from_hex(hash_hex).unwrap(),
        from: Address::from_hex(addr_hex).unwrap(),
        to: Some(Address::from_hex("0x0000000000000000000000000000000000000099").unwrap()),
        value: Wei::new(1_000_000_000_000_000_000),
    }
}

#[given(
    regex = r#"^the account transactions feed knows (\d+) executed txs for "(0x[0-9a-fA-F]{40})"$"#
)]
async fn account_tx_feed_knows_n(world: &mut AppWorld, count: u32, addr_hex: String) {
    let address = Address::from_hex(&addr_hex).unwrap();
    let hashes = [
        "0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa",
        "0xbbbb016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394bb",
        "0xcccc016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394cc",
    ];
    let txs: Vec<AccountTx> = (0..count as usize)
        .map(|i| sample_account_tx(21_000_000 - i as u64, hashes[i % hashes.len()], &addr_hex))
        .collect();
    for tx in &txs {
        seed_tx_reader(world, tx.tx_hash);
    }
    world.account_transactions_stub.set_page(
        address,
        AccountTxPage {
            txs,
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
    let account_tx = world.account_transactions_stub.clone();
    let screen = spawn_address_detail_with_transfers(
        Chain::Ethereum,
        addr,
        reader,
        transfers,
        account_tx,
        tx_reader,
    );
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

// "the user switches to the Transactions tab" is defined in
// block_detail.rs and simply sends a Tab key press to whatever screen
// is on top, so it already drives the AddressDetailScreen correctly.

#[when("the user switches to the Portfolio tab")]
async fn switches_to_portfolio_tab(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    // Overview -> Transactions -> Transfers -> Portfolio (three Tabs from Overview).
    press_key(stack, KeyCode::Tab);
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
    let account_tx = world.account_transactions_stub.clone();
    let portfolio = world.portfolio_stub.clone();
    let tx_reader = world.tx_reader_stub.clone();
    let token_reader = world.token_reader_stub.clone();
    let screen = spawn_address_detail_with_full_feeds(
        Chain::Ethereum,
        addr,
        reader,
        transfers,
        account_tx,
        portfolio,
        tx_reader,
        token_reader,
    );
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

#[then(regex = r#"^once loaded, the Portfolio tab lists (\d+) holdings$"#)]
async fn portfolio_tab_lists_n(world: &mut AppWorld, expected: u32) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).holdings().is_some()).await;
    let screen = current(stack);
    assert_eq!(screen.active_tab(), AddressTab::Portfolio);
    let count = screen.holdings().map(|h| h.len()).unwrap_or(0);
    assert_eq!(count, expected as usize);
}

#[then("once loaded, the Portfolio tab reports no holdings")]
async fn portfolio_tab_empty(world: &mut AppWorld) {
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

#[then("once loaded, the tab bar includes the Impl tab")]
async fn tab_bar_has_impl(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current(s)
            .contract_overview()
            .map(|c| c.proxy.is_some())
            .unwrap_or(false)
    })
    .await;
    let tabs = current(stack).tabs();
    assert!(
        tabs.contains(&AddressTab::ContractImpl),
        "Impl tab missing from {tabs:?}"
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

#[then(regex = r#"^once loaded, the Transactions tab lists (\d+) executed transactions$"#)]
async fn transactions_tab_lists_executed(world: &mut AppWorld, expected: u32) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current(s)
            .account_transactions()
            .map(|p| !p.txs.is_empty())
            .unwrap_or(false)
    })
    .await;
    let screen = current(stack);
    assert_eq!(screen.active_tab(), AddressTab::Transactions);
    let count = screen
        .account_transactions()
        .map(|p| p.txs.len())
        .unwrap_or(0);
    assert_eq!(count, expected as usize);
}

#[when("the user switches to the Transfers tab")]
async fn switches_to_transfers_tab(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    press_key(stack, KeyCode::Tab);
    press_key(stack, KeyCode::Tab);
}

#[then(regex = r#"^once loaded, the Transfers tab lists (\d+) transfers$"#)]
async fn transfers_tab_lists_n(world: &mut AppWorld, expected: u32) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current(s)
            .transfers()
            .map(|p| !p.events.is_empty())
            .unwrap_or(false)
    })
    .await;
    let screen = current(stack);
    assert_eq!(screen.active_tab(), AddressTab::Transfers);
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
    let account_tx = world.account_transactions_stub.clone();
    let portfolio = world.portfolio_stub.clone();
    let tx_reader = world.tx_reader_stub.clone();
    let token_reader = world.token_reader_stub.clone();
    let prices = world.prices_stub.clone();
    let screen = spawn_address_detail_with_erc20_probe(
        Chain::Ethereum,
        addr,
        reader,
        transfers,
        account_tx,
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

#[when("the user presses e on the Portfolio tab")]
async fn presses_e_on_portfolio(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    assert_eq!(current(stack).active_tab(), AddressTab::Portfolio);
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
    regex = r#"^once loaded, the Portfolio tab shows a USD total of "\$([0-9.]+)" and (\d+) token[s]? not priced$"#
)]
async fn portfolio_tab_shows_usd_total(
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

#[then(regex = r#"^the Portfolio tab renders at least (\d+) distribution chart rows$"#)]
async fn portfolio_tab_renders_chart(world: &mut AppWorld, expected_rows: usize) {
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

#[then(regex = r#"^the clipboard sink holds a Portfolio CSV with (\d+) data rows$"#)]
async fn clipboard_has_portfolio_csv(world: &mut AppWorld, expected_rows: usize) {
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

// ---------------------------------------------------------------------------
// Steps migrated from the deleted token_detail.rs (plan/16 §8.1).
// These prime the stubs shared with `spawn_address_detail_with_erc20_probe`
// and with the Token sub-tab scenarios.
// ---------------------------------------------------------------------------

fn parse_window_label(label: &str) -> PriceWindow {
    match label {
        "1d" => PriceWindow::D1,
        "1m" => PriceWindow::M1,
        "1y" => PriceWindow::Y1,
        other => panic!("unknown window {other}"),
    }
}

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
    regex = r#"^the prices stub returns (\d+) points for window "([^"]+)" on "(0x[0-9a-fA-F]{40})"$"#
)]
async fn prices_stub_has_history(
    world: &mut AppWorld,
    n: u64,
    window_label: String,
    addr_hex: String,
) {
    let addr = Address::from_hex(&addr_hex).unwrap();
    let window = parse_window_label(&window_label);
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

// ---------------------------------------------------------------------------
// Contract sub-tab step definitions (migrated from the deleted
// contract_detail.rs). See plan/16 §8.1.
// ---------------------------------------------------------------------------

use blockexplorer_tui::adapters::ui::ContractSubTab;
use blockexplorer_tui::domain::{
    ContractSource, DecodedValue, LogEntry, NetworkStatus, ProxyInfo, ProxyKind, SourceFile,
};

fn sample_source() -> ContractSource {
    ContractSource {
        is_verified: true,
        contract_name: "Storage".into(),
        compiler_version: "v0.8.19+commit.7dd6d404".into(),
        optimizer_enabled: true,
        optimizer_runs: 200,
        evm_version: "paris".into(),
        license: "MIT License (MIT)".into(),
        abi: r#"[{"type":"function","name":"value","inputs":[],"outputs":[{"type":"uint256"}],"stateMutability":"view"}]"#
            .into(),
        files: vec![SourceFile {
            path: "Storage.sol".into(),
            content: "contract Storage { uint256 public value; }".into(),
        }],
        implementation: None,
    }
}

#[given(
    regex = r#"^the proxy detector reports EIP-1967 implementation "(0x[0-9a-fA-F]{40})" for "(0x[0-9a-fA-F]{40})"$"#
)]
async fn proxy_reports_impl(world: &mut AppWorld, impl_hex: String, proxy_hex: String) {
    let impl_addr = Address::from_hex(&impl_hex).unwrap();
    let proxy_addr = Address::from_hex(&proxy_hex).unwrap();
    world
        .proxy_detector_stub
        .set(proxy_addr, ProxyInfo::eip1967_slot(impl_addr));
}

#[given(
    regex = r#"^the contract source stub has a verified single-file source for "(0x[0-9a-fA-F]{40})"$"#
)]
async fn stub_has_verified_single_file(world: &mut AppWorld, addr_hex: String) {
    let addr = Address::from_hex(&addr_hex).unwrap();
    world
        .contract_source_stub
        .insert_source(addr, sample_source());
}

#[given(
    regex = r#"^the contract reader stub returns uint (\d+) for "([^"]+)" on "(0x[0-9a-fA-F]{40})"$"#
)]
async fn reader_stub_returns_uint(
    world: &mut AppWorld,
    value: u128,
    signature: String,
    addr_hex: String,
) {
    let addr = Address::from_hex(&addr_hex).unwrap();
    world
        .contract_reader_stub
        .set_result(addr, &signature, vec![DecodedValue::Uint(value)]);
}

#[given(
    regex = r#"^the contract reader stub reverts with "([^"]+)" for "([^"]+)" on "(0x[0-9a-fA-F]{40})"$"#
)]
async fn reader_stub_reverts(
    world: &mut AppWorld,
    reason: String,
    signature: String,
    addr_hex: String,
) {
    let addr = Address::from_hex(&addr_hex).unwrap();
    world
        .contract_reader_stub
        .set_revert(addr, &signature, &reason);
}

#[given(regex = r#"^the event log stub has (\d+) Transfer events for "(0x[0-9a-fA-F]{40})"$"#)]
async fn event_log_stub_has_transfers(world: &mut AppWorld, count: u32, addr_hex: String) {
    let addr = Address::from_hex(&addr_hex).unwrap();
    let topic: [u8; 32] =
        hex::decode("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
            .unwrap()
            .try_into()
            .unwrap();
    let logs: Vec<LogEntry> = (0..count as usize)
        .map(|_| LogEntry {
            address: addr,
            topics: vec![topic],
            data: Vec::new(),
        })
        .collect();
    world.event_log_stub.set_logs(addr, logs);
}

#[given(
    regex = r#"^the storage stub returns the u128 value (\d+) at slot (\d+) for "(0x[0-9a-fA-F]{40})"$"#
)]
async fn storage_stub_returns_u128(
    world: &mut AppWorld,
    value: u128,
    slot_index: u128,
    addr_hex: String,
) {
    let addr = Address::from_hex(&addr_hex).unwrap();
    let mut slot = [0u8; 32];
    slot[16..].copy_from_slice(&slot_index.to_be_bytes());
    let mut word = [0u8; 32];
    word[16..].copy_from_slice(&value.to_be_bytes());
    world.storage_stub.set_value(addr, slot, word);
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

#[when(regex = r#"^the user opens AddressDetail as contract for "(0x[0-9a-fA-F]{40})"$"#)]
async fn opens_address_detail_as_contract(world: &mut AppWorld, addr_hex: String) {
    use crate::steps::search::spawn_contract_detail;
    build_stack(world);
    let addr = Address::from_hex(&addr_hex).unwrap();
    let reader = world.address_reader_stub.clone();
    let detector = world.proxy_detector_stub.clone();
    let screen = spawn_contract_detail(Chain::Ethereum, addr, reader, detector);
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

#[when(
    regex = r#"^the user opens AddressDetail as contract with source for "(0x[0-9a-fA-F]{40})"$"#
)]
async fn opens_address_detail_as_contract_with_source(world: &mut AppWorld, addr_hex: String) {
    use crate::steps::search::spawn_address_detail_as_contract_full;
    build_stack(world);
    let addr = Address::from_hex(&addr_hex).unwrap();
    let reader = world.address_reader_stub.clone();
    let detector = world.proxy_detector_stub.clone();
    let source = world.contract_source_stub.clone();
    let contract_reader = world.contract_reader_stub.clone();
    let event_log = world.event_log_stub.clone();
    let storage = world.storage_stub.clone();
    let network_status = world.network_stub.clone();
    let screen = spawn_address_detail_as_contract_full(
        Chain::Ethereum,
        addr,
        reader,
        detector,
        source,
        contract_reader,
        event_log,
        storage,
        network_status,
    );
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

#[when(
    regex = r#"^the user opens AddressDetail as contract with Read wiring for "(0x[0-9a-fA-F]{40})"$"#
)]
async fn opens_address_detail_as_contract_with_read(world: &mut AppWorld, addr_hex: String) {
    opens_address_detail_as_contract_with_source(world, addr_hex).await;
}

#[when(
    regex = r#"^the user opens AddressDetail as contract with all wiring for "(0x[0-9a-fA-F]{40})"$"#
)]
async fn opens_address_detail_as_contract_with_all(world: &mut AppWorld, addr_hex: String) {
    // Prime the network-status stub with a deterministic head so
    // load_contract_events_page can compute a concrete 5_000-block
    // window (mirrors the old contract_detail step).
    world.network_stub.set_snapshot(NetworkStatus {
        chain: Chain::Ethereum,
        latest_block: BlockNumber::new(20_000),
        base_fee: Wei::new(0),
        block_time_avg_ms: 12_000,
    });
    opens_address_detail_as_contract_with_source(world, addr_hex).await;
}

#[when(regex = r#"^the user opens AddressDetail as token for "(0x[0-9a-fA-F]{40})"$"#)]
async fn opens_address_detail_as_token(world: &mut AppWorld, addr_hex: String) {
    use crate::steps::search::spawn_token_detail;
    build_stack(world);
    let addr = Address::from_hex(&addr_hex).unwrap();
    let reader = world.token_reader_stub.clone();
    let address_reader = world.address_reader_stub.clone();
    let screen = spawn_token_detail(Chain::Ethereum, addr, reader, address_reader);
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

#[then("once the contract is loaded, no proxy is detected")]
async fn no_proxy(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).contract_overview().is_some()).await;
    let ov = current(stack).contract_overview().expect("loaded");
    assert!(ov.proxy.is_none());
}

#[then(regex = r#"^once the contract is loaded, the proxy points at "(0x[0-9a-fA-F]{40})"$"#)]
async fn proxy_points_at(world: &mut AppWorld, expected_hex: String) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current(s)
            .contract_overview()
            .is_some_and(|ov| ov.proxy.is_some())
    })
    .await;
    let ov = current(stack).contract_overview().expect("loaded");
    let info = ov.proxy.expect("proxy detected");
    assert_eq!(info.kind, ProxyKind::Eip1967);
    assert_eq!(info.implementation.to_hex(), expected_hex);
}

#[then("once the contract source is loaded, the verified flag is true")]
async fn source_verified_flag_true(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).source().is_some()).await;
    let src = current(stack).source().expect("source loaded");
    assert!(src.is_verified);
}

#[then(regex = r#"^the Source sub-tab lists (\d+) file$"#)]
async fn source_subtab_lists_n_files(world: &mut AppWorld, expected: u32) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).source().is_some()).await;
    let screen = current(stack);
    let count = screen.source().map(|s| s.files.len()).unwrap_or(0);
    assert_eq!(count, expected as usize);
}

#[then("once the contract is loaded, the contract source is unavailable")]
async fn contract_source_unavailable(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).contract_overview().is_some()).await;
    for _ in 0..20 {
        stack.top_mut().expect("stack").tick();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(current(stack).source().is_none());
}

fn press_key_bracket(stack: &mut ScreenStack) {
    press_key(stack, KeyCode::Char(']'));
}

async fn cycle_to_contract_subtab(stack: &mut ScreenStack, target: ContractSubTab) {
    tick_until(stack, |s| current(s).active_tab() == AddressTab::Contract).await;
    for _ in 0..12 {
        if current(stack).active_contract_sub() == target {
            return;
        }
        press_key_bracket(stack);
    }
    assert_eq!(current(stack).active_contract_sub(), target);
}

#[when("the user switches to the Read sub-tab")]
async fn switches_to_read_subtab(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    cycle_to_contract_subtab(stack, ContractSubTab::Read).await;
    tick_until(stack, |s| current(s).source().is_some()).await;
}

#[when("the user switches to the Events sub-tab")]
async fn switches_to_events_subtab(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    cycle_to_contract_subtab(stack, ContractSubTab::Events).await;
}

#[when("the user switches to the Storage sub-tab")]
async fn switches_to_storage_subtab(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    cycle_to_contract_subtab(stack, ContractSubTab::Storage).await;
}

#[when("the user selects the first function and executes it")]
async fn selects_first_and_executes(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).source().is_some()).await;
    press_key(stack, KeyCode::Home);
    press_key(stack, KeyCode::Enter);
}

#[then(regex = r#"^once executed, the Read sub-tab shows the uint result (\d+)$"#)]
async fn read_subtab_shows_uint(world: &mut AppWorld, expected: u128) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).last_result_matches_uint(expected)).await;
    assert!(current(stack).last_result_matches_uint(expected));
}

#[then(regex = r#"^once executed, the Read sub-tab reports a revert with "([^"]+)"$"#)]
async fn read_subtab_reports_revert(world: &mut AppWorld, expected_reason: String) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current(s).last_result_is_error_containing(&expected_reason)
    })
    .await;
    assert!(current(stack).last_result_is_error_containing(&expected_reason));
}

#[then(regex = r#"^once loaded, the Events sub-tab lists (\d+) events$"#)]
async fn events_subtab_lists_n(world: &mut AppWorld, expected: u32) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).events_count().is_some()).await;
    let count = current(stack).events_count().unwrap_or(0);
    assert_eq!(count, expected as usize);
}

#[when(regex = r#"^the user presses "(n|N)" on the Events sub-tab$"#)]
async fn presses_n_on_events_subtab(world: &mut AppWorld, key: String) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).events_count().is_some()).await;
    let ch = key.chars().next().unwrap();
    press_key(stack, KeyCode::Char(ch));
    tick_until(stack, |s| current(s).events_count().is_some()).await;
}

#[then(regex = r#"^once reloaded, the Events sub-tab window moved backwards by (\d+) blocks$"#)]
async fn events_subtab_window_moved_back(world: &mut AppWorld, delta: u64) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).events_count().is_some()).await;
    let (from, to) = current(stack).events_window().expect("window loaded");
    assert_eq!(to, 20_000 - delta);
    assert_eq!(from, 20_000 - delta - 4_999);
}

#[then(regex = r#"^once reloaded, the Events sub-tab page offset is (\d+)$"#)]
async fn events_subtab_offset_eq(world: &mut AppWorld, expected: u32) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).events_count().is_some()).await;
    assert_eq!(current(stack).events_offset(), expected);
}

#[when("the user presses Enter on the Storage sub-tab")]
async fn presses_enter_on_storage_subtab(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    press_key(stack, KeyCode::Enter);
}

#[then(regex = r#"^once loaded, the Storage sub-tab shows the value (\d+)$"#)]
async fn storage_subtab_shows_value(world: &mut AppWorld, expected: u128) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).storage_value_u128().is_some()).await;
    let got = current(stack).storage_value_u128().unwrap();
    assert_eq!(got, expected);
}

// ---------------------------------------------------------------------------
// Token sub-tab step definitions
// ---------------------------------------------------------------------------

#[then(
    regex = r#"^once the token probe completes, the Token Overview shows symbol "([^"]+)" and supply (\d+)$"#
)]
async fn token_overview_shows_symbol_and_supply(
    world: &mut AppWorld,
    expected_symbol: String,
    expected_supply: u128,
) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).token_overview().is_some()).await;
    let ov = current(stack).token_overview().expect("loaded");
    assert_eq!(ov.metadata.symbol, expected_symbol);
    assert_eq!(ov.total_supply, expected_supply);
}

#[when("the user switches to the Token Chart sub-tab")]
async fn switches_to_token_chart_subtab(world: &mut AppWorld) {
    use blockexplorer_tui::adapters::ui::{AddressTab, TokenSubTab};

    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).tabs().contains(&AddressTab::Token)).await;
    for _ in 0..8 {
        if current(stack).active_tab() == AddressTab::Token {
            break;
        }
        press_key(stack, KeyCode::Tab);
    }
    assert_eq!(current(stack).active_tab(), AddressTab::Token);
    for _ in 0..6 {
        if current(stack).active_token_sub() == TokenSubTab::Chart {
            break;
        }
        press_key(stack, KeyCode::Char(']'));
    }
    assert_eq!(current(stack).active_token_sub(), TokenSubTab::Chart);
}

#[when(regex = r#"^the user presses "([123])" to select window "([^"]+)"$"#)]
async fn presses_digit_to_select_window(world: &mut AppWorld, digit: char, _window: String) {
    let stack = world.stack.as_mut().expect("stack");
    press_key(stack, KeyCode::Char(digit));
}

#[then(regex = r#"^the active window is "([^"]+)"$"#)]
async fn active_window_is(world: &mut AppWorld, label: String) {
    let stack = world.stack.as_ref().expect("stack");
    assert_eq!(current(stack).active_window(), parse_window_label(&label));
}

#[then(regex = r#"^once the feeds complete, the chart holds (\d+) points for window "([^"]+)"$"#)]
async fn chart_holds_points(world: &mut AppWorld, n: usize, label: String) {
    let expected_window = parse_window_label(&label);
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current(s)
            .token_series()
            .map(|series| series.window == expected_window && series.points.len() == n)
            .unwrap_or(false)
    })
    .await;
    let series = current(stack).token_series().expect("series present");
    assert_eq!(series.window, expected_window);
    assert_eq!(series.points.len(), n);
}

#[then(
    regex = r#"^once the feeds complete, the Token price row renders "\(not indexed by ([^)]+)\)"$"#
)]
async fn token_price_renders_unsupported(world: &mut AppWorld, provider: String) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        matches!(
            current(s).token_price_lookup(),
            PriceLookup::Unsupported { .. }
        )
    })
    .await;
    match current(stack).token_price_lookup() {
        PriceLookup::Unsupported { provider: got } => assert_eq!(*got, provider),
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[when(regex = r#"^the user presses "c" to view as contract$"#)]
async fn presses_c_to_view_as_contract(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).is_incomplete_badge_active()).await;
    press_key(stack, KeyCode::Char('c'));
}

#[then(regex = r#"^the active main tab is "([^"]+)"$"#)]
async fn active_main_tab_is(world: &mut AppWorld, label: String) {
    let stack = world.stack.as_ref().expect("stack");
    let expected = match label.as_str() {
        "Overview" => AddressTab::Overview,
        "Transactions" => AddressTab::Transactions,
        "Transfers" => AddressTab::Transfers,
        "Portfolio" => AddressTab::Portfolio,
        "Token" => AddressTab::Token,
        "Contract" => AddressTab::Contract,
        "Impl" => AddressTab::ContractImpl,
        other => panic!("unknown main tab label {other}"),
    };
    assert_eq!(current(stack).active_tab(), expected);
}
