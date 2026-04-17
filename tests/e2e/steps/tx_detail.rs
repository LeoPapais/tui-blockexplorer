//! Step definitions for the Transaction Detail feature.
//!
//! See `plan/4-tx-detail.md` section 12.3. Scenarios live in
//! `tests/e2e/features/tx_detail.feature`.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{ScreenStack, TxDetailScreen, TxTab},
    domain::{
        Address, BlockHash, BlockNumber, Chain, Transaction, TxHash, TxStatus, TxType, Wei,
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
        block_number: BlockNumber::new(21_345_678),
        block_hash: BlockHash::from_hex(
            "0xaaaa000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap(),
        tx_index: 0,
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(
            Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        ),
        value: Wei::new(0),
        gas_price: Wei::new(14_000_000_000),
        gas_used: 52_341,
        gas_limit: 80_000,
        nonce: 42,
        tx_type: TxType::DynamicFee,
        input: vec![0xa9, 0x05, 0x9c, 0xbb],
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

// ---------------------------------------------------------------------------
// Then
// ---------------------------------------------------------------------------

#[then(regex = r#"^once the transaction is loaded, the Overview tab shows status "([^"]+)"$"#)]
async fn overview_status(world: &mut AppWorld, expected: String) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| current_tx_detail(s).current().is_some()).await;
    let screen = current_tx_detail(stack);
    assert_eq!(screen.active_tab(), TxTab::Overview);
    let tx = screen.current().expect("loaded");
    let actual_status_line = match &tx.status {
        TxStatus::Success => "success".to_string(),
        TxStatus::Failed { reason: Some(r) } => format!("failed - {r}"),
        TxStatus::Failed { reason: None } => "failed".to_string(),
    };
    assert_eq!(actual_status_line, expected);
}
