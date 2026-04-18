//! Step definitions for the Gas Tracker feature.
//!
//! See `plan/9-gas-tracker.md` section 11.1.

use blockexplorer_tui::{
    adapters::ui::{GasTrackerScreen, ScreenStack, gas_feed},
    domain::{Chain, GasSnapshot, Gwei},
};
use cucumber::{given, then, when};

use crate::{steps::search::build_stack, world::AppWorld};

fn current(stack: &ScreenStack) -> &GasTrackerScreen {
    stack
        .top()
        .expect("stack non-empty")
        .as_any()
        .downcast_ref::<GasTrackerScreen>()
        .expect("top of stack must be a GasTrackerScreen")
}

#[given(regex = r#"^the gas stub snapshot has slow (\d+) average (\d+) fast (\d+)$"#)]
async fn gas_stub_snapshot(world: &mut AppWorld, slow: u128, average: u128, fast: u128) {
    let chain = *world.active_chain.get_or_insert(Chain::Ethereum);
    let snapshot = GasSnapshot {
        chain,
        slow: Gwei::new(slow),
        average: Gwei::new(average),
        fast: Gwei::new(fast),
        base_fee: Gwei::new(11),
        trend: vec![Gwei::new(11); 20],
    };
    world.gas_stub.set_snapshot(snapshot);
}

#[when("the user opens the Gas Tracker")]
async fn opens_gas_tracker(world: &mut AppWorld) {
    build_stack(world);
    // Build the Gas Tracker screen primed from the gas stub's latest
    // snapshot; the tests do not need a periodic refresh task.
    let chain = *world.active_chain.get_or_insert(Chain::Ethereum);
    let initial = Some(world.gas_stub.expected(chain));
    let (feed, _sender) = gas_feed();
    let screen = GasTrackerScreen::new(chain, initial, feed);
    let stack = world.stack.as_mut().unwrap();
    stack.push(Box::new(screen));
}

#[then(regex = r#"^the Gas Tracker shows slow (\d+) average (\d+) fast (\d+)$"#)]
async fn shows_tiers(world: &mut AppWorld, slow: u128, average: u128, fast: u128) {
    let stack = world.stack.as_ref().expect("stack");
    let g = current(stack).current().expect("primed");
    assert_eq!(g.slow.value(), slow);
    assert_eq!(g.average.value(), average);
    assert_eq!(g.fast.value(), fast);
}
