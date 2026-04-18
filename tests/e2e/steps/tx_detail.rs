//! Step definitions for the Transaction Detail feature.
//!
//! See `plan/4-tx-detail.md` section 12.3. Scenarios live in
//! `tests/e2e/features/tx_detail.feature`.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{ScreenStack, TxDetailScreen, TxTab, tx_feed},
    application::{TxView, use_cases::load_tx_overview},
    domain::{
        Address, BlockHash, BlockNumber, Chain, ContractAbi, LogEntry, Transaction, TxHash,
        TxStatus, TxType, Wei,
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
        to: Some(
            Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        ),
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
    world.tx_reader_stub.insert(tx);
}

#[given(
    regex = r#"^the tx reader knows tx "(0x[0-9a-fA-F]{64})" failed with reason "([^"]+)"$"#
)]
async fn reader_knows_failure(world: &mut AppWorld, hash_hex: String, reason: String) {
    let tx = sample_tx(
        &hash_hex,
        TxStatus::Failed {
            reason: Some(reason),
        },
    );
    world.tx_reader_stub.insert(tx);
}

#[given(regex = r#"^the tx reader knows tx "(0x[0-9a-fA-F]{64})" is pending$"#)]
async fn reader_knows_pending(world: &mut AppWorld, hash_hex: String) {
    let mut tx = sample_tx(&hash_hex, TxStatus::Pending);
    tx.block_number = None;
    tx.block_hash = None;
    tx.tx_index = None;
    tx.gas_used = None;
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

#[given(regex = r#"^the tx reader knows tx "(0x[0-9a-fA-F]{64})" emitted a Transfer event$"#)]
async fn reader_knows_transfer_event(world: &mut AppWorld, hash_hex: String) {
    let mut tx = sample_tx(&hash_hex, TxStatus::Success);
    let topic: [u8; 32] = hex::decode(
        "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
    )
    .unwrap()
    .try_into()
    .unwrap();
    tx.logs.push(LogEntry {
        address: Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        topics: vec![topic],
        data: Vec::new(),
    });
    world.tx_reader_stub.insert(tx);
}

#[given("the signature directory resolves the Transfer event topic")]
async fn sigdb_has_transfer_event(world: &mut AppWorld) {
    let topic: [u8; 32] = hex::decode(
        "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
    )
    .unwrap()
    .try_into()
    .unwrap();
    world
        .signatures_stub
        .set_event_topic(topic, "Transfer(address,address,uint256)");
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

#[when(regex = r#"^the user opens TxDetail with decoding for hash "(0x[0-9a-fA-F]{64})"$"#)]
async fn opens_tx_detail_with_decoding(world: &mut AppWorld, hash_hex: String) {
    build_stack(world);
    let chain = world.active_chain.expect("chain");
    let hash = TxHash::from_hex(&hash_hex).unwrap();
    let reader = world.tx_reader_stub.clone();
    let contract_source = world.contract_source_stub.clone();
    let signatures = world.signatures_stub.clone();
    let screen = spawn_tx_detail_with_decoding(chain, hash, reader, contract_source, signatures);
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

fn spawn_tx_detail_with_decoding(
    chain: Chain,
    hash: TxHash,
    reader: crate::support::stubs::StubTxReaderPort,
    contract_source: crate::support::stubs::StubContractSourcePort,
    signatures: crate::support::stubs::StubSignatureDirectoryPort,
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

#[then(
    regex = r#"^once the transaction is loaded, the decoded method is "([^"]+)" from ABI$"#
)]
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
    regex = r#"^once the transaction is loaded, the Logs tab decodes "([^"]+)"$"#
)]
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
