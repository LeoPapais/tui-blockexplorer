//! Cucumber `World` shared across every scenario.
//!
//! Holds the stubbed ports, the active-chain hint, the current
//! [`HomeSession`] and a tiny in-memory [`ScreenStack`] so the search
//! scenarios can assert on navigation.

use std::fmt;

use blockexplorer_tui::{
    adapters::ui::{HomeScreen, ScreenStack},
    application::HomeSession,
    domain::{Address, Chain, TxHash},
    infra::search_feed::SearchCache,
};
use cucumber::World;

use crate::support::stubs::FrozenClock;

use crate::support::stubs::{
    StubAddressLookupPort, StubAddressReaderPort, StubBlockLookupPort, StubBlockReaderPort,
    StubChainRegistry, StubContractReaderPort, StubContractSourcePort, StubEnsResolverPort,
    StubEventLogPort, StubGasOraclePort, StubNetworkStatusPort, StubNewHeadsStreamPort,
    StubAccountTransactionsPort, StubPortfolioPort, StubPricesPort, StubProxyDetectionPort,
    StubSignatureDirectoryPort, StubStoragePort, StubTokenPriceStreamPort, StubTokenReaderPort,
    StubTokenSearchPort, StubTransfersPort, StubTxLookupPort, StubTxReaderPort,
    StubTxSimulationPort, StubTxTracePort,
};

pub type AppHomeSession = HomeSession<StubNetworkStatusPort, StubGasOraclePort, StubChainRegistry>;

#[derive(Default, World)]
pub struct AppWorld {
    /// Home-screen stubs.
    pub network_stub: StubNetworkStatusPort,
    pub gas_stub: StubGasOraclePort,
    pub chain_registry: StubChainRegistry,

    /// `newHeads` WebSocket stub. Scenarios that cover the WS path
    /// subscribe through this stub and push events into the receiver
    /// held by the session (see `tests/e2e/steps/home.rs`).
    pub new_heads_stub: StubNewHeadsStreamPort,

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

    /// Contract-source (Etherscan ABI) stub used by the tx-detail
    /// decoding scenarios.
    pub contract_source_stub: StubContractSourcePort,

    /// Signature-directory stub used by the tx-detail decoding
    /// scenarios that only care about "signature resolved somehow"
    /// without pinning the provenance.
    pub signatures_stub: StubSignatureDirectoryPort,

    /// Primary directory (openchain) used by the composite-backed
    /// scenarios. See `plan/15-backlog.md` section 3.2.
    pub openchain_stub: StubSignatureDirectoryPort,

    /// Fallback directory (Samczsun) used by the composite-backed
    /// scenarios.
    pub samczsun_stub: StubSignatureDirectoryPort,

    /// Asset-change simulation stub for the Asset Changes tab.
    pub tx_simulation_stub: StubTxSimulationPort,

    /// State-diff trace stub for the State Changes tab.
    pub tx_trace_stub: StubTxTracePort,

    /// Hash captured by the latest "the tx reader knows tx ... "
    /// step. Used by follow-up `Given`s to attach simulator /
    /// tracer data without repeating the hash in natural language.
    pub last_tx_hash: Option<TxHash>,

    /// Address-detail stub.
    pub address_reader_stub: StubAddressReaderPort,

    /// Unified transfers stub feeding the **Transfers** tab (asset transfers).
    pub transfers_stub: StubTransfersPort,

    /// Normal transactions (`txlist`) stub feeding the **Transactions** tab.
    pub account_transactions_stub: StubAccountTransactionsPort,

    /// Portfolio stub feeding the Tokens tab on Address Detail.
    pub portfolio_stub: StubPortfolioPort,

    /// Address captured by the latest "the transfers feed knows ..."
    /// step. Currently unused beyond bookkeeping but handy for
    /// follow-up Givens that attach more data without repeating the
    /// address.
    pub last_address: Option<Address>,

    /// Contract-detail proxy stub.
    pub proxy_detector_stub: StubProxyDetectionPort,

    /// Contract-reader stub used by the Read tab scenarios.
    pub contract_reader_stub: StubContractReaderPort,

    /// Event-log stub used by the Events tab scenarios.
    pub event_log_stub: StubEventLogPort,

    /// Storage stub used by the Storage tab scenarios.
    pub storage_stub: StubStoragePort,

    /// Token-detail stub.
    pub token_reader_stub: StubTokenReaderPort,

    /// Token-detail prices stub (spot + historical price series).
    pub prices_stub: StubPricesPort,

    /// Live token-price stream stub. Retained for future scenarios
    /// (the Token sub-tab spawners do not subscribe today; see
    /// plan/16 §5).
    #[allow(dead_code)]
    pub price_stream_stub: StubTokenPriceStreamPort,

    pub active_chain: Option<Chain>,
    pub home: Option<AppHomeSession>,

    /// HomeScreen instance driven by the first-run-banner scenario
    /// (plan/10-settings.md §12.2). Kept separate from
    /// [`Self::home`] because the banner scenario drives the screen
    /// directly through its `Screen` impl; the rest of Home's
    /// scenarios go through `HomeSession` + the pure `home::render`
    /// function.
    pub home_screen: Option<HomeScreen>,

    /// Screen stack driven by the search scenarios. Empty before the
    /// Home screen is built, a single-element stack once Home is on
    /// top.
    pub stack: Option<ScreenStack>,

    /// Mirrors `Transition::should_exit()` from the latest
    /// `ScreenStack::apply_command` call: `true` once the dispatcher
    /// would leave the event loop. Used by the Home Esc / q
    /// scenarios to assert that Esc never exits and that q still
    /// does. See `plan/12-screen-runtime.md` §7.1.
    pub stack_exited: bool,

    /// Optional shared TTL cache for the search feed. The default
    /// scenarios keep it `None`; the repeat-within-TTL scenario sets
    /// it via a Given step. See `plan/2-search.md` §12.4.
    pub search_cache: Option<SearchCache<FrozenClock>>,

    /// Block used by the "user opens BlockDetail" step across
    /// scenarios that pre-seed their own block shape (Polygon
    /// signer, Blobs / Withdrawals, ...). When `None` the step
    /// falls back to its hard-coded Polygon fixture so existing
    /// scenarios keep passing. See `plan/3-block-detail.md` §12.5.
    pub pending_block_detail: Option<blockexplorer_tui::domain::Block>,
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
