//! Step definitions for the Transaction Detail feature.
//!
//! See `plan/4-tx-detail.md` section 12.3. Scenarios live in
//! `tests/e2e/features/tx_detail.feature`.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::{
        signatures::CompositeSignatureDirectory,
        ui::{ScreenStack, TxDetailScreen, TxTab, tx_feed},
    },
    application::{LoadStatus, SignatureSource, TxView, use_cases::load_tx_overview},
    domain::{
        Address, AddressStateDiff, AssetChange, AssetChangeKind, AssetKind, BlockHash, BlockNumber,
        CallKind, CallNode, Chain, ContractAbi, DiffChange, LogEntry, ProxyInfo, ProxyKind,
        StateDiff, Transaction, TxHash, TxStatus, TxType, Wei,
    },
};
use cucumber::{given, then, when};

use crate::{
    steps::search::{build_stack, spawn_tx_detail},
    world::AppWorld,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sample_tx(hash_hex: &str, status: TxStatus) -> Transaction {
    Transaction {
        chain: Chain::Ethereum,
        hash: TxHash::from_hex(hash_hex).unwrap(),
        status,
        block_number: Some(BlockNumber::new(21_345_678)),
        block_hash: Some(
            BlockHash::from_hex(
                "0xaaaa000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
        ),
        tx_index: Some(0),
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()),
        value: Wei::new(0),
        gas_price: Wei::new(14_000_000_000),
        gas_used: Some(52_341),
        gas_limit: 80_000,
        nonce: 42,
        tx_type: TxType::DynamicFee,
        input: vec![0xa9, 0x05, 0x9c, 0xbb],
        logs: Vec::new(),
        raw_json: format!("{{\"hash\": \"{hash_hex}\"}}"),
    }
}

fn current_tx_detail(stack: &ScreenStack) -> &TxDetailScreen {
    stack
        .top()
        .expect("stack non-empty")
        .as_any()
        .downcast_ref::<TxDetailScreen>()
        .expect("top of stack must be a TxDetailScreen")
}

async fn tick_until<F>(stack: &mut ScreenStack, mut predicate: F)
where
    F: FnMut(&ScreenStack) -> bool,
{
    for _ in 0..50 {
        let cmd = stack.top_mut().expect("stack non-empty").tick();
        // Apply commands emitted by the tick — currently only
        // Command::None is expected from TxDetailScreen, but keep
        // this guarded for future use.
        match cmd {
            blockexplorer_tui::adapters::ui::Command::None
            | blockexplorer_tui::adapters::ui::Command::Refresh => {}
            other => panic!("unexpected command from tick: {other:?}"),
        }
        if predicate(stack) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

// ---------------------------------------------------------------------------
// Given — prime stubs
// ---------------------------------------------------------------------------

#[given(regex = r#"^the tx reader knows tx "(0x[0-9a-fA-F]{64})" was successful$"#)]
async fn reader_knows_success(world: &mut AppWorld, hash_hex: String) {
    let tx = sample_tx(&hash_hex, TxStatus::Success);
    world.last_tx_hash = Some(tx.hash);
    world.tx_reader_stub.insert(tx);
}

#[given(regex = r#"^the tx reader knows tx "(0x[0-9a-fA-F]{64})" failed with reason "([^"]+)"$"#)]
async fn reader_knows_failure(world: &mut AppWorld, hash_hex: String, reason: String) {
    let tx = sample_tx(
        &hash_hex,
        TxStatus::Failed {
            reason: Some(reason),
        },
    );
    world.last_tx_hash = Some(tx.hash);
    world.tx_reader_stub.insert(tx);
}

#[given(regex = r#"^the tx reader knows tx "(0x[0-9a-fA-F]{64})" is pending$"#)]
async fn reader_knows_pending(world: &mut AppWorld, hash_hex: String) {
    let mut tx = sample_tx(&hash_hex, TxStatus::Pending);
    tx.block_number = None;
    tx.block_hash = None;
    tx.tx_index = None;
    tx.gas_used = None;
    world.last_tx_hash = Some(tx.hash);
    world.tx_reader_stub.insert(tx);
}

#[given(regex = r#"^the contract source knows the ABI of "(0x[0-9a-fA-F]{40})"$"#)]
async fn contract_source_has_abi(world: &mut AppWorld, addr_hex: String) {
    let address = Address::from_hex(&addr_hex).unwrap();
    let abi = ContractAbi {
        abi: r#"[{"type":"function","name":"transfer","inputs":[{"name":"to","type":"address"},{"name":"value","type":"uint256"}],"outputs":[]}]"#
            .to_string(),
        is_verified: true,
    };
    world.contract_source_stub.insert(address, abi);
}

#[given(
    regex = r#"^the contract source knows the proxy ABI of "(0x[0-9a-fA-F]{40})" has no transfer$"#
)]
async fn contract_source_has_proxy_abi(world: &mut AppWorld, addr_hex: String) {
    let address = Address::from_hex(&addr_hex).unwrap();
    let abi = ContractAbi {
        abi: r#"[{"type":"function","name":"implementation","inputs":[],"outputs":[{"name":"","type":"address"}]},{"type":"function","name":"admin","inputs":[],"outputs":[{"name":"","type":"address"}]}]"#
            .to_string(),
        is_verified: true,
    };
    world.contract_source_stub.insert(address, abi);
}

#[given(regex = r#"^the contract source knows the implementation ABI of "(0x[0-9a-fA-F]{40})"$"#)]
async fn contract_source_has_impl_abi(world: &mut AppWorld, addr_hex: String) {
    let address = Address::from_hex(&addr_hex).unwrap();
    let abi = ContractAbi {
        abi: r#"[{"type":"function","name":"transfer","inputs":[{"name":"to","type":"address"},{"name":"value","type":"uint256"}],"outputs":[{"name":"","type":"bool"}]}]"#
            .to_string(),
        is_verified: true,
    };
    world.contract_source_stub.insert(address, abi);
}

#[given(
    regex = r#"^the proxy detector maps "(0x[0-9a-fA-F]{40})" to implementation "(0x[0-9a-fA-F]{40})"$"#
)]
async fn proxy_detector_maps(world: &mut AppWorld, proxy_hex: String, impl_hex: String) {
    let proxy = Address::from_hex(&proxy_hex).unwrap();
    let implementation = Address::from_hex(&impl_hex).unwrap();
    world.proxy_detector_stub.set(
        proxy,
        ProxyInfo {
            kind: ProxyKind::Eip1967,
            implementation,
        },
    );
}

#[given(regex = r#"^the tx reader knows tx "(0x[0-9a-fA-F]{64})" emitted a Transfer event$"#)]
async fn reader_knows_transfer_event(world: &mut AppWorld, hash_hex: String) {
    let mut tx = sample_tx(&hash_hex, TxStatus::Success);
    let topic: [u8; 32] =
        hex::decode("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
            .unwrap()
            .try_into()
            .unwrap();
    tx.logs.push(LogEntry {
        address: Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        topics: vec![topic],
        data: Vec::new(),
    });
    world.last_tx_hash = Some(tx.hash);
    world.tx_reader_stub.insert(tx);
}

#[given("the signature directory resolves the Transfer event topic")]
async fn sigdb_has_transfer_event(world: &mut AppWorld) {
    let topic: [u8; 32] =
        hex::decode("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
            .unwrap()
            .try_into()
            .unwrap();
    world
        .signatures_stub
        .set_event_topic(topic, "Transfer(address,address,uint256)");
}

#[given(regex = r"^the simulator reports 1 native ETH transfer for that tx$")]
async fn simulator_reports_one_transfer(world: &mut AppWorld) {
    let hash = last_tx_hash(world);
    let from = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    let to = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    world.tx_simulation_stub.set_changes(
        hash,
        vec![AssetChange {
            kind: AssetChangeKind::Transfer,
            asset: AssetKind::Native,
            from: Some(from),
            to: Some(to),
            amount: Wei::new(1_000_000_000_000_000_000),
        }],
    );
}

#[given(regex = r"^the tracer reports that the state-diff is unsupported$")]
async fn tracer_unsupported(world: &mut AppWorld) {
    world.tx_trace_stub.mark_unsupported();
}

#[given("the tracer is slow")]
async fn tracer_is_slow(world: &mut AppWorld) {
    world
        .tx_trace_stub
        .set_delay(std::time::Duration::from_millis(500));
}

#[given("the simulator is slow")]
async fn simulator_is_slow(world: &mut AppWorld) {
    world
        .tx_simulation_stub
        .set_delay(std::time::Duration::from_millis(500));
}

#[then("the Overview tab loads before the tracer stub has been called")]
async fn overview_loads_before_tracer(world: &mut AppWorld) {
    // The spawn_full task delivers the base view first, then awaits
    // sim / tracer. We drain the channel until the base view lands
    // and assert the tracer stub is still untouched.
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current_tx_detail(s).current().is_some()).await;
    // The tracer is delayed by 500ms, so if the base view lands,
    // the tracer stub has not yet been invoked.
    assert_eq!(
        world.tx_trace_stub.call_count(),
        0,
        "tracer must not be called before the base view is delivered"
    );
}

#[given("the tracer exposes a call tree with one staticcall child")]
async fn tracer_exposes_call_tree(world: &mut AppWorld) {
    let hash = last_tx_hash(world);
    let from = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    let to = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    let child_to = Address::from_hex("0x1111111111111111111111111111111111111111").unwrap();
    let tree = CallNode {
        kind: CallKind::Call,
        from,
        to: Some(to),
        value: Wei::new(0),
        input: Vec::new(),
        output: Vec::new(),
        gas_used: 52_341,
        error: None,
        children: vec![CallNode {
            kind: CallKind::Staticcall,
            from: to,
            to: Some(child_to),
            value: Wei::new(0),
            input: Vec::new(),
            output: Vec::new(),
            gas_used: 128,
            error: None,
            children: Vec::new(),
        }],
    };
    world.tx_trace_stub.set_call_tree(hash, tree);
}

#[then(regex = r"^once the transaction is loaded, the Internal tab renders (\d+) call frames$")]
async fn internal_tab_renders_n_frames(world: &mut AppWorld, expected: u32) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        matches!(
            current_tx_detail(s).current().map(|v| &v.call_tree),
            Some(LoadStatus::Loaded(_)) | Some(LoadStatus::Unsupported)
        )
    })
    .await;
    let screen = current_tx_detail(stack);
    let view = screen.current().expect("loaded");
    match &view.call_tree {
        LoadStatus::Loaded(root) => assert_eq!(root.frame_count(), expected as usize),
        other => panic!("expected Loaded, got {other:?}"),
    }
}

#[given(regex = r"^the tracer reports a balance diff for the sender$")]
async fn tracer_has_balance_diff(world: &mut AppWorld) {
    let hash = last_tx_hash(world);
    let from = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    world.tx_trace_stub.set_state_diff(
        hash,
        StateDiff {
            entries: vec![AddressStateDiff {
                address: from,
                balance: DiffChange::Changed {
                    from: "0xde0b6b3a7640000".into(),
                    to: "0xde0b6b3a7630000".into(),
                },
                nonce: DiffChange::Unchanged,
                code: DiffChange::Unchanged,
                storage: Vec::new(),
            }],
        },
    );
}

fn last_tx_hash(world: &AppWorld) -> TxHash {
    world
        .last_tx_hash
        .expect("scenario must provide the tx hash via the tx reader Given step")
}

// ---------------------------------------------------------------------------
// When
// ---------------------------------------------------------------------------

#[when(regex = r#"^the user opens TxDetail for hash "(0x[0-9a-fA-F]{64})"$"#)]
async fn opens_tx_detail(world: &mut AppWorld, hash_hex: String) {
    build_stack(world);
    let chain = world.active_chain.expect("chain");
    let hash = TxHash::from_hex(&hash_hex).unwrap();
    let reader = world.tx_reader_stub.clone();
    let screen = spawn_tx_detail(chain, hash, reader);
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

#[when(regex = r#"^the user opens TxDetail with full enrichment for hash "(0x[0-9a-fA-F]{64})"$"#)]
async fn opens_tx_detail_with_full_enrichment(world: &mut AppWorld, hash_hex: String) {
    build_stack(world);
    let chain = world.active_chain.expect("chain");
    let hash = TxHash::from_hex(&hash_hex).unwrap();
    let reader = world.tx_reader_stub.clone();
    let contract_source = world.contract_source_stub.clone();
    let signatures = world.signatures_stub.clone();
    let sim = world.tx_simulation_stub.clone();
    let tracer = world.tx_trace_stub.clone();
    let detector = world.proxy_detector_stub.clone();
    let screen = spawn_tx_detail_with_full_enrichment(
        chain,
        hash,
        reader,
        contract_source,
        signatures,
        detector,
        sim,
        tracer,
    );
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

#[when(regex = r#"^the user opens TxDetail with decoding for hash "(0x[0-9a-fA-F]{64})"$"#)]
async fn opens_tx_detail_with_decoding(world: &mut AppWorld, hash_hex: String) {
    build_stack(world);
    let chain = world.active_chain.expect("chain");
    let hash = TxHash::from_hex(&hash_hex).unwrap();
    let reader = world.tx_reader_stub.clone();
    let contract_source = world.contract_source_stub.clone();
    let signatures = world.signatures_stub.clone();
    let detector = world.proxy_detector_stub.clone();
    let screen =
        spawn_tx_detail_with_decoding(chain, hash, reader, contract_source, signatures, detector);
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

fn spawn_tx_detail_with_decoding(
    chain: Chain,
    hash: TxHash,
    reader: crate::support::stubs::StubTxReaderPort,
    contract_source: crate::support::stubs::StubContractSourcePort,
    signatures: crate::support::stubs::StubSignatureDirectoryPort,
    detector: crate::support::stubs::StubProxyDetectionPort,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    let (feed, sender) = tx_feed();
    tokio::spawn(async move {
        let blockexplorer_tui::adapters::ui::TxFeedSender {
            updates_tx,
            mut input_rx,
        } = sender;
        while let Some(h) = input_rx.recv().await {
            let result = load_tx_overview::run_with_decoding(
                &reader,
                &contract_source,
                &signatures,
                &detector,
                h,
                chain,
            )
            .await;
            if let Ok(view) = result
                && updates_tx.send(view).is_err()
            {
                break;
            }
        }
    });
    Box::new(TxDetailScreen::loading(chain, hash, feed))
}

#[allow(clippy::too_many_arguments)]
fn spawn_tx_detail_with_full_enrichment(
    chain: Chain,
    hash: TxHash,
    reader: crate::support::stubs::StubTxReaderPort,
    contract_source: crate::support::stubs::StubContractSourcePort,
    signatures: crate::support::stubs::StubSignatureDirectoryPort,
    detector: crate::support::stubs::StubProxyDetectionPort,
    sim: crate::support::stubs::StubTxSimulationPort,
    tracer: crate::support::stubs::StubTxTracePort,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    let (feed, sender) = tx_feed();
    // Mirror `infra::tx_feed::spawn_full`: send the base view
    // (reader + ABI + signature directory) before awaiting the
    // slow sim / trace calls. That's what `plan/4-tx-detail.md`
    // section 12.6.1 asserts: the Overview tab must not wait on
    // trace/debug methods.
    tokio::spawn(async move {
        let blockexplorer_tui::adapters::ui::TxFeedSender {
            updates_tx,
            mut input_rx,
        } = sender;
        while let Some(h) = input_rx.recv().await {
            let Ok(mut view) = load_tx_overview::run_with_decoding(
                &reader,
                &contract_source,
                &signatures,
                &detector,
                h,
                chain,
            )
            .await
            else {
                continue;
            };
            // Deliver the base view first.
            if updates_tx.send(view.clone()).is_err() {
                break;
            }
            // Then run the heavier enrichments concurrently.
            let sim = sim.clone();
            let tracer = tracer.clone();
            let mut v_sim = view.clone();
            let mut v_trace = view.clone();
            let mut v_call_tree = view.clone();
            let (a, s, c) = tokio::join!(
                async {
                    load_tx_overview::load_asset_changes(&sim, &mut v_sim, chain).await;
                    v_sim.asset_changes
                },
                async {
                    load_tx_overview::load_state_diff(&tracer, &mut v_trace, chain).await;
                    v_trace.state_diff
                },
                async {
                    load_tx_overview::load_call_tree(&tracer, &mut v_call_tree, chain).await;
                    v_call_tree.call_tree
                },
            );
            view.asset_changes = a;
            view.state_diff = s;
            view.call_tree = c;
            if updates_tx.send(view).is_err() {
                break;
            }
        }
    });
    Box::new(TxDetailScreen::loading(chain, hash, feed))
}

// ---------------------------------------------------------------------------
// Then
// ---------------------------------------------------------------------------

#[then(regex = r#"^once the transaction is loaded, the Overview tab shows status "([^"]+)"$"#)]
async fn overview_status(world: &mut AppWorld, expected: String) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current_tx_detail(s).current().is_some()).await;
    let screen = current_tx_detail(stack);
    assert_eq!(screen.active_tab(), TxTab::Overview);
    let view = screen.current().expect("loaded");
    let actual_status_line = match &view.tx.status {
        TxStatus::Success => "success".to_string(),
        TxStatus::Failed { reason: Some(r) } => format!("failed - {r}"),
        TxStatus::Failed { reason: None } => "failed".to_string(),
        TxStatus::Pending => "pending".to_string(),
    };
    assert_eq!(actual_status_line, expected);
}

#[then(regex = r#"^once the transaction is loaded, the decoded method is "([^"]+)" from ABI$"#)]
async fn overview_method_from_abi(world: &mut AppWorld, expected: String) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current_tx_detail(s)
            .current()
            .and_then(|v| v.decoded_method.as_ref())
            .is_some()
    })
    .await;
    let screen = current_tx_detail(stack);
    let view: &TxView = screen.current().expect("loaded");
    let method = view.decoded_method.as_ref().expect("decoded");
    assert_eq!(method.signature, expected);
    assert_eq!(
        method.source,
        blockexplorer_tui::application::SignatureSource::Abi
    );
}

#[then(
    regex = r#"^once the transaction is loaded, the decoded method is "([^"]+)" via implementation "(0x[0-9a-fA-F]{40})"$"#
)]
async fn overview_method_via_proxy(
    world: &mut AppWorld,
    expected_sig: String,
    expected_impl_hex: String,
) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current_tx_detail(s)
            .current()
            .and_then(|v| v.decoded_method.as_ref())
            .is_some()
    })
    .await;
    let screen = current_tx_detail(stack);
    let view: &TxView = screen.current().expect("loaded");
    let method = view.decoded_method.as_ref().expect("decoded");
    assert_eq!(method.signature, expected_sig);
    match method.source {
        SignatureSource::ProxyAbi { implementation, .. } => {
            let expected_impl = Address::from_hex(&expected_impl_hex).unwrap();
            assert_eq!(implementation, expected_impl);
        }
        other => panic!("expected ProxyAbi, got {other:?}"),
    }
}

#[then(regex = r#"^once the transaction is loaded, the Asset Changes tab lists (\d+) change$"#)]
async fn asset_changes_lists_n(world: &mut AppWorld, expected: u32) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        matches!(
            current_tx_detail(s).current().map(|v| &v.asset_changes),
            Some(LoadStatus::Loaded(_)) | Some(LoadStatus::Unsupported)
        )
    })
    .await;
    let screen = current_tx_detail(stack);
    let view: &TxView = screen.current().expect("loaded");
    match &view.asset_changes {
        LoadStatus::Loaded(changes) => assert_eq!(changes.len(), expected as usize),
        other => panic!("expected Loaded, got {other:?}"),
    }
}

#[then(
    regex = r#"^once the transaction is loaded, the State Changes tab reports it is unsupported$"#
)]
async fn state_changes_unsupported(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        matches!(
            current_tx_detail(s).current().map(|v| &v.state_diff),
            Some(LoadStatus::Unsupported) | Some(LoadStatus::Loaded(_))
        )
    })
    .await;
    let screen = current_tx_detail(stack);
    let view: &TxView = screen.current().expect("loaded");
    assert!(matches!(view.state_diff, LoadStatus::Unsupported));
}

#[then(
    regex = r#"^once the transaction is loaded, the State Changes tab lists (\d+) address(?:es)?$"#
)]
async fn state_changes_lists_n(world: &mut AppWorld, expected: u32) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        matches!(
            current_tx_detail(s).current().map(|v| &v.state_diff),
            Some(LoadStatus::Loaded(_)) | Some(LoadStatus::Unsupported)
        )
    })
    .await;
    let screen = current_tx_detail(stack);
    let view: &TxView = screen.current().expect("loaded");
    match &view.state_diff {
        LoadStatus::Loaded(diff) => assert_eq!(diff.entries.len(), expected as usize),
        other => panic!("expected Loaded, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Composite (openchain + Samczsun) fallback-chain scenarios
// ---------------------------------------------------------------------------
//
// These steps back the `plan/15-backlog.md` section 3.2 scenarios:
//   - "Unknown selector resolves via openchain"
//   - "Openchain miss falls back to Samczsun"
//
// They prime two directory stubs (openchain_stub, samczsun_stub) and
// compose them via `CompositeSignatureDirectory`, so the assertions
// can pin the exact provenance the UI will surface.

const TRANSFER_SELECTOR: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];

#[given("a tx whose target has no verified ABI")]
async fn tx_without_verified_abi(world: &mut AppWorld) {
    // The target contract simply has no entry in the contract-source
    // stub; the stub returns `None` from `get_abi` by default. We
    // still prime a successful tx so the decoding pipeline reaches
    // the directory fallback.
    let hash = "0xeeee016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394ee";
    let tx = sample_tx(hash, TxStatus::Success);
    world.last_tx_hash = Some(tx.hash);
    world.tx_reader_stub.insert(tx);
}

#[given(regex = r#"^openchain returns "([^"]+)" for the selector$"#)]
async fn openchain_returns(world: &mut AppWorld, signature: String) {
    world.openchain_stub.set_selector_with_source(
        TRANSFER_SELECTOR,
        &signature,
        SignatureSource::Openchain,
    );
}

#[given("openchain returns no match for the selector")]
async fn openchain_returns_no_match(_world: &mut AppWorld) {
    // No-op: the openchain stub is empty by default, which the
    // composite reads as `Ok(None)` and then delegates to Samczsun.
}

#[given(regex = r#"^samczsun returns "([^"]+)" for the selector$"#)]
async fn samczsun_returns(world: &mut AppWorld, signature: String) {
    world.samczsun_stub.set_selector_with_source(
        TRANSFER_SELECTOR,
        &signature,
        SignatureSource::Samczsun,
    );
}

#[when("the user opens TxDetail")]
async fn opens_tx_detail_via_composite(world: &mut AppWorld) {
    build_stack(world);
    let chain = world.active_chain.expect("chain");
    let hash = last_tx_hash(world);
    let reader = world.tx_reader_stub.clone();
    let contract_source = world.contract_source_stub.clone();
    let openchain = world.openchain_stub.clone();
    let samczsun = world.samczsun_stub.clone();
    let screen =
        spawn_tx_detail_with_composite(chain, hash, reader, contract_source, openchain, samczsun);
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

#[then("the Overview tab shows that signature sourced from the directory")]
async fn overview_signature_sourced_from_directory(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current_tx_detail(s)
            .current()
            .and_then(|v| v.decoded_method.as_ref())
            .is_some()
    })
    .await;
    let screen = current_tx_detail(stack);
    let view: &TxView = screen.current().expect("loaded");
    let method = view.decoded_method.as_ref().expect("decoded");
    assert_eq!(method.signature, "transfer(address,uint256)");
    assert_eq!(method.source, SignatureSource::Openchain);
}

#[then("the Overview tab shows that signature with provenance samczsun")]
async fn overview_signature_with_samczsun_provenance(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current_tx_detail(s)
            .current()
            .and_then(|v| v.decoded_method.as_ref())
            .is_some()
    })
    .await;
    let screen = current_tx_detail(stack);
    let view: &TxView = screen.current().expect("loaded");
    let method = view.decoded_method.as_ref().expect("decoded");
    assert_eq!(method.signature, "transfer(address,uint256)");
    assert_eq!(method.source, SignatureSource::Samczsun);
}

fn spawn_tx_detail_with_composite(
    chain: Chain,
    hash: TxHash,
    reader: crate::support::stubs::StubTxReaderPort,
    contract_source: crate::support::stubs::StubContractSourcePort,
    openchain: crate::support::stubs::StubSignatureDirectoryPort,
    samczsun: crate::support::stubs::StubSignatureDirectoryPort,
) -> Box<dyn blockexplorer_tui::adapters::ui::Screen> {
    let (feed, sender) = tx_feed();
    let signatures = CompositeSignatureDirectory::new(openchain, samczsun);
    // The composite fallback-chain scenarios do not exercise proxy
    // detection; an empty stub keeps the cascade moving straight
    // from direct ABI into the signature directory.
    let proxy_detector = crate::support::stubs::StubProxyDetectionPort::default();
    tokio::spawn(async move {
        let blockexplorer_tui::adapters::ui::TxFeedSender {
            updates_tx,
            mut input_rx,
        } = sender;
        while let Some(h) = input_rx.recv().await {
            let result = load_tx_overview::run_with_decoding(
                &reader,
                &contract_source,
                &signatures,
                &proxy_detector,
                h,
                chain,
            )
            .await;
            if let Ok(view) = result
                && updates_tx.send(view).is_err()
            {
                break;
            }
        }
    });
    Box::new(TxDetailScreen::loading(chain, hash, feed))
}

/// Wait for the view to load, then press `s`. Mirrors the helper
/// pattern used by the Overview / Logs tab steps above.
#[when(regex = r#"^once the transaction is loaded, the user presses "s"$"#)]
async fn once_loaded_press_s(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current_tx_detail(s).current().is_some()).await;
    let key = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('s'),
        crossterm::event::KeyModifiers::NONE,
    );
    stack.top_mut().expect("stack non-empty").handle_key(key);
}

#[then(regex = r#"^the tx detail screen has observed (\d+) re-simulation$"#)]
async fn tx_detail_resimulate_count(world: &mut AppWorld, expected: u32) {
    let stack = world.stack.as_ref().expect("stack");
    let screen = current_tx_detail(stack);
    assert_eq!(screen.resimulate_count(), expected);
}

#[then(regex = r#"^once the transaction is loaded, the Logs tab decodes "([^"]+)"$"#)]
async fn logs_tab_decodes(world: &mut AppWorld, expected: String) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current_tx_detail(s)
            .current()
            .map(|v| !v.decoded_logs.is_empty())
            .unwrap_or(false)
    })
    .await;
    let screen = current_tx_detail(stack);
    let view: &TxView = screen.current().expect("loaded");
    let log = &view.decoded_logs[0];
    let sig = log.signature.as_ref().expect("decoded");
    assert_eq!(sig.signature, expected);
}
