//! Step definitions for the Contract Detail feature.
//!
//! See `plan/7-contract-detail.md` section 12.3.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{Command, ContractDetailScreen, ContractFeedSender, ContractTab, ScreenStack, contract_feed},
    application::{
        ports::{AddressReaderPort, ContractSourcePort, ProxyDetectionPort},
        use_cases::load_contract_overview,
    },
    domain::{Address, Chain, ContractSource, ProxyInfo, ProxyKind, SourceFile},
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

// ---------------------------------------------------------------------------
// Source / ABI tabs (plan 7 section 12.4.1)
// ---------------------------------------------------------------------------

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
    regex = r#"^the contract source stub has a verified single-file source for "(0x[0-9a-fA-F]{40})"$"#
)]
async fn stub_has_verified_single_file(world: &mut AppWorld, addr_hex: String) {
    let addr = Address::from_hex(&addr_hex).unwrap();
    world.contract_source_stub.insert_source(addr, sample_source());
}

#[when(regex = r#"^the user opens ContractDetail with source for "(0x[0-9a-fA-F]{40})"$"#)]
async fn opens_contract_detail_with_source(world: &mut AppWorld, addr_hex: String) {
    build_stack(world);
    let addr = Address::from_hex(&addr_hex).unwrap();
    let reader = world.address_reader_stub.clone();
    let detector = world.proxy_detector_stub.clone();
    let source = world.contract_source_stub.clone();
    let screen = spawn_contract_detail_with_source(Chain::Ethereum, addr, reader, detector, source);
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

fn spawn_contract_detail_with_source<
    R: AddressReaderPort + Clone + 'static,
    P: ProxyDetectionPort + Clone + 'static,
    S: ContractSourcePort + Clone + 'static,
>(
    chain: Chain,
    address: Address,
    reader: R,
    detector: P,
    source: S,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    let (feed, sender) = contract_feed();
    tokio::spawn(async move {
        let ContractFeedSender {
            updates_tx,
            source_tx,
            mut input_rx,
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            if let Ok(ov) = load_contract_overview::run(&reader, &detector, addr, chain).await
                && updates_tx.send(ov).is_err()
            {
                break;
            }
            if let Ok(Some(src)) = source.get_source(addr, chain).await
                && source_tx.send(src).is_err()
            {
                break;
            }
        }
    });
    Box::new(ContractDetailScreen::loading(chain, address, feed))
}

#[then("once the contract source is loaded, the verified flag is true")]
async fn source_verified_flag_true(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).source().is_some()).await;
    let src = current(stack).source().expect("source loaded");
    assert!(src.is_verified);
}

#[then(regex = r#"^the Source tab lists (\d+) file$"#)]
async fn source_tab_lists_n_files(world: &mut AppWorld, expected: u32) {
    let stack = world.stack.as_mut().expect("stack");
    // Cycle Overview -> Source
    press_tab(stack);
    tick_until(stack, |s| current(s).source().is_some()).await;
    let screen = current(stack);
    assert_eq!(screen.active_tab(), ContractTab::Source);
    let count = screen.source().map(|s| s.files.len()).unwrap_or(0);
    assert_eq!(count, expected as usize);
}

#[then("the ABI tab is populated")]
async fn abi_tab_populated(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    // Cycle Source -> ABI
    press_tab(stack);
    let screen = current(stack);
    assert_eq!(screen.active_tab(), ContractTab::Abi);
    let abi = screen.source().map(|s| s.abi.as_str()).unwrap_or("");
    assert!(!abi.is_empty());
}

#[then("once the contract is loaded, the source is unavailable")]
async fn source_unavailable(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current(s).current().is_some()).await;
    // Give the source fetch a few extra ticks to "not arrive".
    for _ in 0..20 {
        stack.top_mut().expect("stack").tick();
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(current(stack).source().is_none());
}

fn press_tab(stack: &mut ScreenStack) {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let screen = stack.top_mut().expect("stack non-empty");
    let cmd = screen.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    match cmd {
        Command::None | Command::Refresh => {}
        other => panic!("unexpected command: {other:?}"),
    }
}
