//! Step definitions for the Token Detail feature.
//!
//! See `plan/8-token-detail.md` section 12.3.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{Command, ScreenStack, TokenDetailScreen},
    domain::{Address, Chain, TokenMetadata, TokenOverview},
};
use cucumber::{given, then, when};

use crate::{
    steps::search::{build_stack, spawn_token_detail},
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
    });
}

#[when(regex = r#"^the user opens TokenDetail for "(0x[0-9a-fA-F]{40})"$"#)]
async fn opens_token_detail(world: &mut AppWorld, addr_hex: String) {
    build_stack(world);
    let addr = Address::from_hex(&addr_hex).unwrap();
    let reader = world.token_reader_stub.clone();
    let screen = spawn_token_detail(Chain::Ethereum, addr, reader);
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

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
    // Give the resolver a chance; it should never populate current()
    // because the stub returns None.
    let loaded = tick_until(stack, |s| current(s).current().is_some()).await;
    assert!(!loaded, "token was unexpectedly loaded");
    assert!(current(stack).current().is_none());
}
