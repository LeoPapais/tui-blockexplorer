//! Step definitions for the Address Detail feature.
//!
//! See `plan/6-address-detail.md` section 12.3. Scenarios live in
//! `tests/e2e/features/address_detail.feature`.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{AddressDetailScreen, AddressTab, Command, ScreenStack},
    domain::{
        Address, AddressKind, AddressOverview, BlockNumber, Chain, TransferAsset,
        TransferCategory, TransferEvent, TransferPage, TxHash, Wei,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use cucumber::{given, then, when};

use crate::{
    steps::search::{build_stack, spawn_address_detail, spawn_address_detail_with_transfers},
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
    world.address_reader_stub.insert(AddressOverview {
        chain: Chain::Ethereum,
        address: Address::from_hex(addr_hex).unwrap(),
        balance: Wei::new(balance),
        nonce,
        kind,
        ens_name: None,
    });
}

#[given(
    regex = r#"^the address reader knows EOA "(0x[0-9a-fA-F]{40})" with balance (\d+) and nonce (\d+)$"#
)]
async fn reader_knows_eoa(
    world: &mut AppWorld,
    addr_hex: String,
    balance: u128,
    nonce: u64,
) {
    insert_overview(world, &addr_hex, AddressKind::Eoa, balance, nonce);
}

#[given(
    regex = r#"^the address reader knows contract "(0x[0-9a-fA-F]{40})" with balance (\d+) and nonce (\d+)$"#
)]
async fn reader_knows_contract(
    world: &mut AppWorld,
    addr_hex: String,
    balance: u128,
    nonce: u64,
) {
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
        AddressKind::Eoa => "EOA",
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
        to: Some(
            Address::from_hex("0x0000000000000000000000000000000000000099").unwrap(),
        ),
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
    match cmd {
        Command::None | Command::Refresh => {}
        Command::Pop => {
            stack.pop();
        }
        Command::Push(next) => {
            stack.push(next);
        }
        Command::Replace(next) => {
            stack.pop();
            stack.push(next);
        }
        Command::Quit => {
            stack.clear();
        }
    }
}
