//! Step definitions for the Home screen.
//!
//! Scenarios live in `tests/e2e/features/home.feature`. Behaviour is
//! specified in [`plan/1-home.md`](../../../../plan/1-home.md).
//!
//! All steps currently fail with `unimplemented!()` so the harness reports
//! every scenario as red until the Home use cases are wired up in a later
//! phase.

use cucumber::{given, then, when};

use crate::world::AppWorld;

// ---------------------------------------------------------------------------
// Given
// ---------------------------------------------------------------------------

#[given("the user launches the app")]
async fn user_launches_the_app(_world: &mut AppWorld) {
    unimplemented!(
        "see plan/1-home.md section 4 - launch wiring not yet implemented"
    );
}

#[given(regex = r#"^the active chain is "([^"]+)"$"#)]
async fn active_chain_is(world: &mut AppWorld, chain: String) {
    // Setting the chain on the world is safe before the rest is wired, but
    // the scenario will still fail on later steps.
    world.active_chain = Some(chain);
}

#[given("the Home screen is rendered")]
async fn home_is_rendered(_world: &mut AppWorld) {
    unimplemented!(
        "see plan/1-home.md section 2 - Home rendering not yet implemented"
    );
}

#[given(regex = r#"^the Home screen is rendered with "([^"]+)"$"#)]
async fn home_is_rendered_with(_world: &mut AppWorld, _chain: String) {
    unimplemented!(
        "see plan/1-home.md section 4.3 - SwitchChain use case not yet implemented"
    );
}

// ---------------------------------------------------------------------------
// When
// ---------------------------------------------------------------------------

#[when("the Home screen is rendered")]
async fn when_home_is_rendered(_world: &mut AppWorld) {
    unimplemented!(
        "see plan/1-home.md section 2 - Home rendering not yet implemented"
    );
}

#[when(regex = r#"^a new "newHeads" event is pushed from the stub$"#)]
async fn newheads_pushed(_world: &mut AppWorld) {
    unimplemented!(
        "see plan/1-home.md section 4.1 - NetworkStatusPort subscription not yet implemented"
    );
}

#[when(regex = r#"^the user presses "c"$"#)]
async fn user_presses_c(_world: &mut AppWorld) {
    unimplemented!(
        "see plan/1-home.md section 3 - chain picker keybinding not yet implemented"
    );
}

#[when(regex = r#"^selects "([^"]+)"$"#)]
async fn selects_chain(_world: &mut AppWorld, _chain: String) {
    unimplemented!(
        "see plan/1-home.md section 4.3 - SwitchChain use case not yet implemented"
    );
}

#[when(regex = r#"^the "newHeads" subscription drops$"#)]
async fn subscription_drops(_world: &mut AppWorld) {
    unimplemented!(
        "see plan/1-home.md section 7 - degraded state handling not yet implemented"
    );
}

// ---------------------------------------------------------------------------
// Then
// ---------------------------------------------------------------------------

#[then("the Network card shows the latest block number from the stub")]
async fn network_card_shows_block_number(_world: &mut AppWorld) {
    unimplemented!(
        "see plan/1-home.md section 2 - Network card render not yet implemented"
    );
}

#[then("the Gas Tracker card shows slow, average and fast gwei values")]
async fn gas_card_shows_values(_world: &mut AppWorld) {
    unimplemented!(
        "see plan/1-home.md section 4.2 - ObserveGasOracle not yet implemented"
    );
}

#[then("the Network card updates the latest block number")]
async fn network_card_updates(_world: &mut AppWorld) {
    unimplemented!(
        "see plan/1-home.md section 4.1 - ObserveNetworkStatus not yet implemented"
    );
}

#[then("the Gas Tracker card recomputes its values")]
async fn gas_card_recomputes(_world: &mut AppWorld) {
    unimplemented!(
        "see plan/1-home.md section 4.2 - ObserveGasOracle not yet implemented"
    );
}

#[then(regex = r#"^the active chain becomes "([^"]+)"$"#)]
async fn active_chain_becomes(_world: &mut AppWorld, _chain: String) {
    unimplemented!(
        "see plan/1-home.md section 4.3 - SwitchChain use case not yet implemented"
    );
}

#[then(regex = r#"^the Network card reflects the latest block number for "([^"]+)"$"#)]
async fn network_card_reflects_chain(_world: &mut AppWorld, _chain: String) {
    unimplemented!(
        "see plan/1-home.md section 4.1 - per-chain NetworkStatus not yet implemented"
    );
}

#[then(regex = r#"^the header shows a "disconnected" badge$"#)]
async fn header_shows_disconnected(_world: &mut AppWorld) {
    unimplemented!(
        "see plan/1-home.md section 7 - degraded state banner not yet implemented"
    );
}

#[then("the app schedules a reconnect")]
async fn app_schedules_reconnect(_world: &mut AppWorld) {
    unimplemented!(
        "see .cursor/rules/external-apis.mdc - reconnect policy not yet implemented"
    );
}
