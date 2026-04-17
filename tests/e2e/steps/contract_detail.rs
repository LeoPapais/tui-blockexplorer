//! Step definitions for the Contract Detail feature.
//!
//! See `plan/7-contract-detail.md` section 12.3.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{Command, ContractDetailScreen, ScreenStack},
    domain::{Address, Chain, ProxyInfo, ProxyKind},
};
use cucumber::{given, then, when};

use crate::{
    steps::search::{build_stack, spawn_contract_detail},
    world::AppWorld,
};

fn current(stack: &ScreenStack) -> &ContractDetailScreen {
    stack
        .top()
        .expect("stack non-empty")
        .as_any()
        .downcast_ref::<ContractDetailScreen>()
        .expect("top of stack must be a ContractDetailScreen")
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

#[given(
    regex = r#"^the proxy detector reports EIP-1967 implementation "(0x[0-9a-fA-F]{40})" for "(0x[0-9a-fA-F]{40})"$"#
)]
async fn proxy_reports_impl(
    world: &mut AppWorld,
    impl_hex: String,
    proxy_hex: String,
) {
    let impl_addr = Address::from_hex(&impl_hex).unwrap();
    let proxy_addr = Address::from_hex(&proxy_hex).unwrap();
    world.proxy_detector_stub.set(
        proxy_addr,
        ProxyInfo {
            kind: ProxyKind::Eip1967,
            implementation: impl_addr,
        },
    );
}

#[when(regex = r#"^the user opens ContractDetail for "(0x[0-9a-fA-F]{40})"$"#)]
async fn opens_contract_detail(world: &mut AppWorld, addr_hex: String) {
    build_stack(world);
    let addr = Address::from_hex(&addr_hex).unwrap();
    let reader = world.address_reader_stub.clone();
    let detector = world.proxy_detector_stub.clone();
    let screen = spawn_contract_detail(Chain::Ethereum, addr, reader, detector);
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

#[then("once the contract is loaded, no proxy is detected")]
async fn no_proxy(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).current().is_some()).await;
    let ov = current(stack).current().expect("loaded");
    assert!(ov.proxy.is_none());
}

#[then(
    regex = r#"^once the contract is loaded, the proxy points at "(0x[0-9a-fA-F]{40})"$"#
)]
async fn proxy_points_at(world: &mut AppWorld, expected_hex: String) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current(s).current().is_some_and(|ov| ov.proxy.is_some())
    })
    .await;
    let ov = current(stack).current().expect("loaded");
    let info = ov.proxy.expect("proxy detected");
    assert_eq!(info.kind, ProxyKind::Eip1967);
    assert_eq!(info.implementation.to_hex(), expected_hex);
}
