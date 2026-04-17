//! Cucumber `World` shared across every scenario.
//!
//! Holds the stubbed ports, the active-chain hint, the current
//! [`HomeSession`] and a tiny in-memory [`ScreenStack`] so the search
//! scenarios can assert on navigation.

use std::fmt;

use blockexplorer_tui::{
    adapters::ui::ScreenStack,
    application::HomeSession,
    domain::Chain,
};
use cucumber::World;

use crate::support::stubs::{
    StubAddressLookupPort, StubAddressReaderPort, StubBlockLookupPort, StubBlockReaderPort,
    StubChainRegistry, StubEnsResolverPort, StubGasOraclePort, StubNetworkStatusPort,
    StubPendingTxStreamPort, StubProxyDetectionPort, StubTokenSearchPort, StubTxLookupPort,
    StubTxReaderPort,
};

pub type AppHomeSession =
    HomeSession<StubNetworkStatusPort, StubGasOraclePort, StubChainRegistry>;

#[derive(Default, World)]
pub struct AppWorld {
    /// Home-screen stubs.
    pub network_stub: StubNetworkStatusPort,
    pub gas_stub: StubGasOraclePort,
    pub chain_registry: StubChainRegistry,

    /// Search-screen stubs.
    pub tx_stub: StubTxLookupPort,
    pub block_stub: StubBlockLookupPort,
    pub address_stub: StubAddressLookupPort,
    pub ens_stub: StubEnsResolverPort,
    pub token_stub: StubTokenSearchPort,

    /// Block-detail stub.
    pub block_reader_stub: StubBlockReaderPort,

    /// Tx-detail stub.
    pub tx_reader_stub: StubTxReaderPort,

    /// Mempool stub.
    pub pending_stub: StubPendingTxStreamPort,

    /// Address-detail stub.
    pub address_reader_stub: StubAddressReaderPort,

    /// Contract-detail proxy stub.
    pub proxy_detector_stub: StubProxyDetectionPort,

    pub active_chain: Option<Chain>,
    pub home: Option<AppHomeSession>,

    /// Screen stack driven by the search scenarios. Empty before the
    /// Home screen is built, a single-element stack once Home is on
    /// top.
    pub stack: Option<ScreenStack>,
}

impl fmt::Debug for AppWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppWorld")
            .field("active_chain", &self.active_chain)
            .field("home_present", &self.home.is_some())
            .field(
                "stack_top",
                &self
                    .stack
                    .as_ref()
                    .and_then(|s| s.top().map(|t| t.title().to_string())),
            )
            .finish()
    }
}
