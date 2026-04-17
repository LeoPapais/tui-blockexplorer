//! Cucumber `World` shared across every scenario.
//!
//! Holds the stubbed ports, the active-chain hint and the current
//! [`HomeSession`]. Filled in as scenarios run; resets for each scenario
//! because cucumber builds a fresh `Default` instance.

use std::fmt;

use blockexplorer_tui::{
    application::HomeSession,
    domain::Chain,
};
use cucumber::World;

use crate::support::stubs::{StubChainRegistry, StubGasOraclePort, StubNetworkStatusPort};

pub type AppHomeSession =
    HomeSession<StubNetworkStatusPort, StubGasOraclePort, StubChainRegistry>;

#[derive(Default, World)]
pub struct AppWorld {
    /// Stub used by every scenario. Cloneable handles are primed by the
    /// `Given` steps and read by the application code.
    pub network_stub: StubNetworkStatusPort,
    pub gas_stub: StubGasOraclePort,
    pub chain_registry: StubChainRegistry,

    /// Which chain the user has selected. Set by the `Given the active
    /// chain is "..."` step.
    pub active_chain: Option<Chain>,

    /// Home session, created when the Home screen is "rendered". Absent
    /// before that step runs.
    pub home: Option<AppHomeSession>,
}

impl fmt::Debug for AppWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppWorld")
            .field("active_chain", &self.active_chain)
            .field("home_present", &self.home.is_some())
            .finish()
    }
}
