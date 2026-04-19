//! Cross-screen step definitions.
//!
//! Every feature file opens with the same handful of Background givens
//! — "the user launches the app", "the user is on Home", "the active
//! chain is …". They used to live inside whichever module was written
//! first (`home.rs` or `search.rs`), which meant editing an unrelated
//! module every time a new feature reused a common given. `shared.rs`
//! is the canonical home for those cross-cutting steps so every feature
//! file pulls from the same place.
//!
//! Scenario-specific givens stay in their screen module. Anything moved
//! here must be:
//!   1. referenced by at least two `*.feature` files, **or**
//!   2. so generic (background-only) that keeping it screen-local
//!      misleads reviewers.
//!
//! Related plan sections: `plan/15-backlog.md` §8.16 "Testing and CI"
//! (shared BDD step library bullet) and `.cursor/rules/testing.mdc`.

use blockexplorer_tui::domain::Chain;
use cucumber::given;

use crate::world::AppWorld;

/// Called from the home Background step. We keep the assertion tight
/// so a test that accidentally primes a session before this step
/// fails with a clear message.
#[given("the user launches the app")]
async fn user_launches_the_app(world: &mut AppWorld) {
    assert!(
        world.home.is_none(),
        "the launch step must run before any Home session is created"
    );
}

/// Matches the Background `Given the user is on Home` that every
/// non-home feature file uses. The search module owns the actual
/// stack-building helper because the `HomeScreen` constructed here
/// needs the search factory; we delegate to `search::build_stack`.
#[given("the user is on Home")]
async fn user_on_home(world: &mut AppWorld) {
    super::search::build_stack(world);
}

/// The active-chain hint that drives every per-chain stub and the
/// initial `HomeSession::new` invocation. `home.rs` still owns the
/// fixture priming; this step only records the hint on the world so
/// feature files that do not open the Home screen (for example the
/// search `Given the active chain is "polygon"` line) can rely on
/// `world.active_chain` before starting any stub-aware step.
#[given(regex = r#"^the active chain is "([^"]+)"$"#)]
async fn active_chain_is(world: &mut AppWorld, chain: String) {
    let chain = Chain::from_slug(&chain).expect("scenario references a known chain");
    world.active_chain = Some(chain);
    super::home::prime_home_fixtures_for(world, chain);
}
