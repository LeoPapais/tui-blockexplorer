//! Step definitions for the Block Detail feature.
//!
//! See `plan/3-block-detail.md` section 11.3. Scenarios live in
//! `tests/e2e/features/block_detail.feature`.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{BlockDetailScreen, BlockTab, Command, ScreenStack},
    domain::{
        Address, Block, BlockHash, BlockId, BlockNumber, BlockSummary, Chain, TxHash,
        UnixTimestamp, Wei,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use cucumber::{given, then, when};

use crate::{
    steps::search::{build_search_factory, build_stack, spawn_block_detail},
    world::AppWorld,
};

// ---------------------------------------------------------------------------
// Fixtures and helpers
// ---------------------------------------------------------------------------

fn sample_block(number: u64, hash_hex: &str, parent_hex: &str, tx_count: usize) -> Block {
    let tx_hashes: Vec<TxHash> = (0..tx_count)
        .map(|i| {
            let last = format!("{i:064x}");
            TxHash::from_hex(&format!("0x{last}")).unwrap()
        })
        .collect();
    Block {
        chain: Chain::Ethereum,
        number: BlockNumber::new(number),
        hash: BlockHash::from_hex(hash_hex).unwrap(),
        parent_hash: BlockHash::from_hex(parent_hex).unwrap(),
        timestamp: UnixTimestamp::from_seconds(1_710_000_000 + number),
        miner: Address::from_hex("0x1111111111111111111111111111111111111111").unwrap(),
        gas_used: 12_000_000,
        gas_limit: 30_000_000,
        base_fee: Some(Wei::new(11_400_000_000)),
        size: 102_400,
        extra_data: vec![0x42, 0x42],
        tx_hashes,
    }
}

fn default_parent_hex() -> String {
    "0x0000000000000000000000000000000000000000000000000000000000000000".to_string()
}

fn main_block_hash_hex() -> String {
    "0xaaaa000000000000000000000000000000000000000000000000000000000000".to_string()
}

fn apply_command(stack: &mut ScreenStack, cmd: Command) {
    match cmd {
        Command::None | Command::Refresh => {}
        Command::Pop => {
            stack.pop();
        }
        Command::Quit => stack.clear(),
        Command::Push(screen) => stack.push(screen),
        Command::Replace(screen) => {
            stack.pop();
            stack.push(screen);
        }
    }
}

fn press(stack: &mut ScreenStack, key: KeyEvent) {
    let cmd = stack.top_mut().expect("stack non-empty").handle_key(key);
    apply_command(stack, cmd);
}

fn tick(stack: &mut ScreenStack) {
    let cmd = stack.top_mut().expect("stack non-empty").tick();
    apply_command(stack, cmd);
}

async fn tick_until<F>(stack: &mut ScreenStack, mut predicate: F)
where
    F: FnMut(&ScreenStack) -> bool,
{
    for _ in 0..50 {
        tick(stack);
        if predicate(stack) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn current_block_detail(stack: &ScreenStack) -> &BlockDetailScreen {
    stack
        .top()
        .expect("stack non-empty")
        .as_any()
        .downcast_ref::<BlockDetailScreen>()
        .expect("top of stack must be a BlockDetailScreen")
}

/// Build a Home->BlockDetail stack primed with a pre-loaded block.
/// Used by scenarios that start with "the user is on BlockDetail...".
fn push_block_detail_directly(world: &mut AppWorld, block: Block) {
    build_stack(world);
    let chain = world.active_chain.expect("chain");
    let block_reader = world.block_reader_stub.clone();
    let tx_reader = world.tx_reader_stub.clone();
    block_reader.insert(block.clone());
    let screen = spawn_block_detail(
        chain,
        BlockId::Number(block.number),
        block_reader,
        tx_reader,
    );
    let stack = world.stack.as_mut().unwrap();
    stack.push(screen);
}

// ---------------------------------------------------------------------------
// Given
// ---------------------------------------------------------------------------

#[given(regex = r#"^the block reader knows block (\d+) with two transactions$"#)]
async fn reader_knows_block_with_txs(world: &mut AppWorld, number: u64) {
    let block = sample_block(number, &main_block_hash_hex(), &default_parent_hex(), 2);
    world.block_reader_stub.insert(block);
}

#[given(regex = r#"^the block reader knows block (\d+) with hash "(0x[0-9a-fA-F]{64})"$"#)]
async fn reader_knows_block_with_hash(world: &mut AppWorld, number: u64, hash_hex: String) {
    let block = sample_block(number, &hash_hex, &default_parent_hex(), 1);
    world.block_reader_stub.insert(block);
}

// "Given the stub knows block <n>" is already registered for the
// Search feature in steps/search.rs. We reuse it as-is to prime the
// BlockSummary side of the Search pipeline.

#[given(regex = r#"^the user is on BlockDetail for block (\d+) with two transactions$"#)]
async fn user_is_on_block_detail(world: &mut AppWorld, number: u64) {
    let block = sample_block(number, &main_block_hash_hex(), &default_parent_hex(), 2);
    world.block_reader_stub.insert(block.clone());
    // Prime the block-lookup stub so Search (if run later) can find
    // it too.
    world.block_stub.insert(BlockSummary {
        number: block.number,
        hash: block.hash,
    });
    push_block_detail_directly(world, block);

    // Give the feed a chance to settle to the initial block.
    let stack = world.stack.as_mut().unwrap();
    tick_until(stack, |s| current_block_detail(s).current().is_some()).await;
}

// ---------------------------------------------------------------------------
// When
// ---------------------------------------------------------------------------

#[when("confirms the block candidate")]
async fn confirm_block_candidate(world: &mut AppWorld) {
    // Install a fresh search factory that routes Block through the
    // BlockDetailScreen. The Home screen we built in the background
    // step used whatever factory was set then; for this scenario we
    // ensure the factory is rebuilt with the current (primed)
    // stubs by pushing the stack as needed.
    let chain = world.active_chain.expect("chain");
    let factory = build_search_factory(world, chain);
    // Re-assign search factory on the bottom-most Home screen. The
    // simplest approach: rebuild the whole stack if Home has no
    // factory yet, otherwise leave it; then press Enter on the
    // current SearchScreen to push the Block screen.
    let stack = world.stack.as_mut().expect("stack");
    // Ignore: we may have opened search before the factory was
    // updated, but the existing factory still captures the block
    // reader stub that scenarios prime before opening search.
    let _ = factory;

    press(stack, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
}

#[when("the user switches to the Transactions tab")]
async fn switches_to_transactions_tab(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    press(stack, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
}

#[when("selects the first transaction and presses Enter")]
async fn first_tx_enter(world: &mut AppWorld) {
    let stack = world.stack.as_mut().expect("stack");
    // `tx_selected` defaults to 0, so Enter is enough.
    press(stack, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
}

#[when(regex = r#"^the user presses "(\[|\])"$"#)]
async fn press_bracket(world: &mut AppWorld, key: String) {
    let stack = world.stack.as_mut().expect("stack");
    let c = key.chars().next().unwrap();
    press(stack, KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
}

// ---------------------------------------------------------------------------
// Then
// ---------------------------------------------------------------------------

#[then(regex = r#"^an? "([^"]+)" screen is on top$"#)]
async fn screen_on_top(world: &mut AppWorld, title: String) {
    let stack = world.stack.as_ref().expect("stack");
    assert_eq!(stack.top().unwrap().title(), title);
}

#[then(regex = r#"^once the block is loaded, the Overview tab shows block (\d+)$"#)]
async fn overview_shows_block(world: &mut AppWorld, number: u64) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current_block_detail(s)
            .current()
            .map(|b| b.number.value() == number)
            .unwrap_or(false)
    })
    .await;
    let top = current_block_detail(stack);
    assert_eq!(top.active_tab(), BlockTab::Overview);
    let block = top.current().expect("loaded");
    assert_eq!(block.number.value(), number);
}

#[then(regex = r#"^BlockDetail eventually shows block (\d+)$"#)]
async fn block_detail_shows(world: &mut AppWorld, number: u64) {
    let stack = world.stack.as_mut().expect("stack");
    tick_until(stack, |s| {
        current_block_detail(s)
            .current()
            .map(|b| b.number.value() == number)
            .unwrap_or(false)
    })
    .await;
    let top = current_block_detail(stack);
    assert_eq!(top.current().unwrap().number.value(), number);
}
