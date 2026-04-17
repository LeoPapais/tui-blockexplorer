//! Step definitions for the Address Detail feature.
//!
//! See `plan/6-address-detail.md` section 12.3. Scenarios live in
//! `tests/e2e/features/address_detail.feature`.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{AddressDetailScreen, Command, ScreenStack},
    domain::{Address, AddressKind, AddressOverview, Chain, Wei},
};
use cucumber::{given, then, when};

use crate::{
    steps::search::{build_stack, spawn_address_detail},
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
