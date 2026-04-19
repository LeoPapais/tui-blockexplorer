//! Composition root.
//!
//! Parses the CLI, loads config, decides whether to run with real
//! Alchemy data or the frozen demo fixture, and finally starts the
//! event loop in a Tokio multi-thread runtime.
//!
//! See `plan/12-screen-runtime.md` (runtime) and
//! `plan/14-config-and-credentials.md` (credentials + wiring).

pub mod address_feed;
mod block_feed;
pub mod config;
mod contract_feed;
mod gas_feed;
pub mod health;
pub mod home_feed;
pub mod logging;
pub mod mempool_feed;
pub mod runtime;
pub mod search_feed;
mod token_feed;
mod tx_feed;

use anyhow::{Context, Result};
use reqwest::Client;
use url::Url;

use std::time::Duration;

use crate::{
    adapters::{
        cache::TtlCache,
        config::InMemoryChainRegistry,
        etherscan::{
            CachedEtherscanProxyHint, EtherscanClient, EtherscanContractSource, EtherscanProxyHint,
            EtherscanTokenSearch,
        },
        prices::{AlchemyPrices, PollingTokenPriceStream, PricesClient},
        rpc::{
            AlchemyAddressLookup, AlchemyAddressReader, AlchemyBlockLookup, AlchemyBlockReader,
            AlchemyContractReader, AlchemyEnsResolver, AlchemyEventLog, AlchemyGasOracleAdapter,
            AlchemyNetworkStatusAdapter, AlchemyPortfolio, AlchemyProxyDetector, AlchemySimulation,
            AlchemyStorage, AlchemyTokenReader, AlchemyTransfers, AlchemyTxLookup, AlchemyTxReader,
            AlchemyTxTracer, CompositeProxyDetector, RpcClient,
        },
        signatures::{
            CompositeSignatureDirectory, HttpSignatureDirectory, SamczsunSignatureDirectory,
        },
        ui::{
            AddressDetailScreen, AppConfigSnapshot, BlockDetailScreen, ContractDetailScreen,
            DetailPlaceholderScreen, GasTrackerScreen, GlobalKeyMap, HelpModal, HomeScreen,
            MempoolScreen, Screen, ScreenStack, SearchScreen, SettingsScreen, TokenDetailScreen,
            TxDetailScreen, address_feed, block_feed, contract_feed, gas_feed, gas_refresh_channel,
            search_feed, token_feed, tx_feed,
        },
    },
    application::{ConnectionStatus, HomeSession, HomeViewModel, ports::PendingTxStreamPort},
    domain::{BlockId, Chain, PendingTxFilter, ResolvedEntity},
    infra::search_feed::SearchCache,
};
use mempool_feed::{EmptyPendingTxStream, spawn_filter_drain};

/// Per-(chain, input) TTL for the search resolution cache. Matches the
/// figure documented in `plan/2-search.md` section 12.4.
const SEARCH_CACHE_TTL: Duration = Duration::from_secs(60);

/// Build a live `TxDetailScreen`. Every tab is populated through the
/// full enrichment pipeline (ABI decoding when possible + asset
/// changes + state diff) so the heavier tabs never get stuck in
/// "pending..." just because ETHERSCAN_API_KEY is missing: we fall
/// back to `TxContractSource::Noop` in that case and the decoding
/// simply returns no matches.
fn live_tx_detail_screen(
    chain: Chain,
    hash: crate::domain::TxHash,
    rpc: RpcClient,
    etherscan_key: Option<String>,
) -> Box<dyn Screen> {
    let reader = AlchemyTxReader::new(rpc.clone());
    let (feed, sender) = tx_feed();

    let sim = AlchemySimulation::new(rpc.clone());
    let trace = AlchemyTxTracer::new(rpc.clone());
    let proxy_detector = build_proxy_detector(rpc.clone(), etherscan_key.as_deref());

    let contract_source = etherscan_key
        .and_then(|key| EtherscanClient::with_default_http(key).ok())
        .map(EtherscanContractSource::new)
        .map(TxContractSource::Etherscan)
        .unwrap_or(TxContractSource::Noop);

    let signatures = build_signature_directory();

    std::mem::drop(tx_feed::spawn_full(
        chain,
        reader,
        contract_source,
        signatures,
        proxy_detector,
        sim,
        trace,
        sender,
    ));

    Box::new(TxDetailScreen::loading(chain, hash, feed))
}

/// Build a live `AddressDetailScreen` backed by address-reader,
/// transfers and portfolio tasks, and wire the Transactions / Tokens
/// tabs to open TxDetail / TokenDetail on Enter.
fn live_address_detail_screen(
    chain: Chain,
    address: crate::domain::Address,
    rpc: RpcClient,
    alchemy_key: String,
    etherscan_key: Option<String>,
) -> Box<dyn Screen> {
    let reader = AlchemyAddressReader::new(rpc.clone());
    let transfers = AlchemyTransfers::new(rpc.clone());
    let portfolio = AlchemyPortfolio::new(rpc.clone());
    let token_reader = AlchemyTokenReader::new(rpc.clone());
    // Prices client can fail to build on a malformed key; degrade
    // gracefully via the same TokenPrices enum used in
    // `live_token_detail_screen`.
    let prices = match PricesClient::with_api_key(&alchemy_key) {
        Ok(client) => TokenPrices::Alchemy(AlchemyPrices::new(client)),
        Err(_) => TokenPrices::Noop,
    };
    // plan/6-address-detail.md §11 "Shipped": wrap the base resolver
    // in the 5-minute TTL decorator (`.cursor/rules/external-apis.mdc`).
    let ens = crate::adapters::ens::CachedEnsResolver::new(AlchemyEnsResolver::new(rpc.clone()));
    let (feed, sender) = address_feed();
    std::mem::drop(address_feed::spawn(
        chain,
        reader,
        transfers,
        portfolio,
        token_reader,
        prices,
        ens,
        sender,
    ));

    let rpc_for_tx = rpc.clone();
    let etherscan_for_tx = etherscan_key.clone();
    let open_tx: crate::adapters::ui::address_detail::OpenTxFactory = Box::new(move |hash| {
        live_tx_detail_screen(chain, hash, rpc_for_tx.clone(), etherscan_for_tx.clone())
    });

    let rpc_for_token = rpc.clone();
    let alchemy_for_token = alchemy_key.clone();
    let etherscan_for_token = etherscan_key.clone();
    let open_token: crate::adapters::ui::address_detail::OpenTokenFactory =
        Box::new(move |contract| {
            live_token_detail_screen(
                chain,
                contract,
                rpc_for_token.clone(),
                &alchemy_for_token,
                etherscan_for_token.clone(),
            )
        });

    let rpc_for_contract = rpc;
    let etherscan_for_contract = etherscan_key;
    let open_contract: crate::adapters::ui::address_detail::OpenContractFactory =
        Box::new(move |addr| {
            live_contract_detail_screen(
                chain,
                addr,
                rpc_for_contract.clone(),
                etherscan_for_contract.clone(),
            )
        });

    Box::new(AddressDetailScreen::with_factories(
        chain,
        address,
        feed,
        Some(open_tx),
        Some(open_token),
        Some(open_contract),
    ))
}

/// Build a live `ContractDetailScreen` backed by address-reader,
/// proxy-detection and Etherscan source tasks.
fn live_contract_detail_screen(
    chain: Chain,
    address: crate::domain::Address,
    rpc: RpcClient,
    etherscan_key: Option<String>,
) -> Box<dyn Screen> {
    let reader = AlchemyAddressReader::new(rpc.clone());
    let detector = build_proxy_detector(rpc.clone(), etherscan_key.as_deref());
    let contract_reader = AlchemyContractReader::new(rpc.clone());
    let event_log = AlchemyEventLog::new(rpc.clone());
    let storage = AlchemyStorage::new(rpc.clone());
    let network_status = AlchemyNetworkStatusAdapter::new(rpc);

    let source = etherscan_key
        .and_then(|key| EtherscanClient::with_default_http(key).ok())
        .map(EtherscanContractSource::new)
        .map(TxContractSource::Etherscan)
        .unwrap_or(TxContractSource::Noop);

    let (feed, sender) = contract_feed();
    std::mem::drop(contract_feed::spawn(
        chain,
        reader,
        detector,
        source,
        contract_reader,
        event_log,
        storage,
        network_status,
        sender,
    ));
    Box::new(ContractDetailScreen::loading(chain, address, feed))
}

/// Build a live `TokenDetailScreen` backed by:
///
/// - `AlchemyTokenReader` (metadata + totalSupply),
/// - `AlchemyPrices` (spot + historical prices, when the key could
///   build a `PricesClient`),
/// - `AlchemyTransfers::get_for_contract` (Transfers tab).
///
/// When the Prices client cannot be constructed (should not happen
/// with a valid key) we fall back to a null implementation so the
/// Overview + Transfers tabs still populate. The Chart tab then
/// renders its "no data" empty state.
fn live_token_detail_screen(
    chain: Chain,
    address: crate::domain::Address,
    rpc: RpcClient,
    alchemy_key: &str,
    etherscan_key: Option<String>,
) -> Box<dyn Screen> {
    let reader = AlchemyTokenReader::new(rpc.clone());
    let transfers = AlchemyTransfers::new(rpc.clone());
    let prices = match PricesClient::with_api_key(alchemy_key) {
        Ok(client) => TokenPrices::Alchemy(AlchemyPrices::new(client)),
        Err(_) => TokenPrices::Noop,
    };

    let (feed, sender) = token_feed();
    // Live price streaming: wrap the Prices port in a polling adapter
    // that emits one sample every `DEFAULT_POLL_INTERVAL`. See
    // `plan/8-token-detail.md` §13.1.
    let stream = PollingTokenPriceStream::with_default_interval(prices.clone());
    std::mem::drop(token_feed::spawn_with_stream(
        chain, reader, prices, transfers, stream, sender,
    ));

    let rpc_for_tx = rpc.clone();
    let etherscan_for_tx = etherscan_key.clone();
    let open_tx: crate::adapters::ui::TokenOpenTxFactory = Box::new(move |hash| {
        live_tx_detail_screen(chain, hash, rpc_for_tx.clone(), etherscan_for_tx.clone())
    });

    // `View as Contract` shortcut surfaced when the metadata comes
    // back too incomplete to treat the address as an ERC-20. See
    // `plan/8-token-detail.md` §13.2.
    let rpc_for_contract = rpc;
    let etherscan_for_contract = etherscan_key;
    let open_contract: crate::adapters::ui::TokenOpenContractFactory = Box::new(move |addr| {
        live_contract_detail_screen(
            chain,
            addr,
            rpc_for_contract.clone(),
            etherscan_for_contract.clone(),
        )
    });

    Box::new(TokenDetailScreen::with_factories(
        chain,
        address,
        feed,
        Some(open_tx),
        Some(open_contract),
    ))
}

/// Wrapper around `AlchemyPrices` that degrades to a Noop when the
/// `PricesClient` cannot be built. Keeps the token feed generic
/// enough to swallow the missing-credentials case without crashing.
#[derive(Clone)]
enum TokenPrices {
    Alchemy(AlchemyPrices),
    Noop,
}

impl crate::application::ports::PricesPort for TokenPrices {
    async fn get_single(
        &self,
        address: crate::domain::Address,
        chain: Chain,
    ) -> Result<crate::domain::PriceLookup, crate::domain::DomainError> {
        match self {
            TokenPrices::Alchemy(inner) => inner.get_single(address, chain).await,
            // plan/15-backlog.md §3.4: when the Prices API is not
            // wired we surface `Unsupported` with a provider label
            // distinct from `alchemy-prices` so the UI can still
            // tell the user why the column is empty.
            TokenPrices::Noop => Ok(crate::domain::PriceLookup::Unsupported { provider: "noop" }),
        }
    }

    async fn get_history(
        &self,
        address: crate::domain::Address,
        chain: Chain,
        window: crate::domain::PriceWindow,
    ) -> Result<crate::domain::PriceSeries, crate::domain::DomainError> {
        match self {
            TokenPrices::Alchemy(inner) => inner.get_history(address, chain, window).await,
            TokenPrices::Noop => Ok(crate::domain::PriceSeries::empty(window)),
        }
    }
}

/// Composite TokenSearchPort used by the Search screen: prefers the
/// Etherscan-backed implementation when an API key is available and
/// degrades to a Noop otherwise. The Noop path preserves today's
/// behaviour (no ticker rows, no errors). See `plan/2-search.md`
/// section 10.2.
#[derive(Clone)]
enum TokenSearchBackend {
    Etherscan(EtherscanTokenSearch),
    Noop,
}

impl crate::application::ports::TokenSearchPort for TokenSearchBackend {
    async fn by_symbol(
        &self,
        symbol: &str,
        chain: Chain,
    ) -> Result<Vec<crate::domain::TokenMetadata>, crate::domain::DomainError> {
        match self {
            TokenSearchBackend::Etherscan(inner) => inner.by_symbol(symbol, chain).await,
            TokenSearchBackend::Noop => Ok(Vec::new()),
        }
    }

    async fn by_name(
        &self,
        text: &str,
        chain: Chain,
    ) -> Result<Vec<crate::domain::TokenMetadata>, crate::domain::DomainError> {
        match self {
            TokenSearchBackend::Etherscan(inner) => inner.by_name(text, chain).await,
            TokenSearchBackend::Noop => Ok(Vec::new()),
        }
    }
}

/// Build the live proxy detector used by Contract Detail and the
/// tx-decoding pipeline: Alchemy slot-probing primary (EIP-1967 +
/// UUPS + Transparent) composed with an optional, TTL-cached
/// Etherscan implementation-hint fallback.
///
/// When the Etherscan key is missing or the client cannot be built
/// (should not happen with a valid key), the composite degrades to
/// the primary detector — callers retain the previous behaviour.
/// See `plan/7-contract-detail.md` sections 12.5.1 and 12.5.2.
fn build_proxy_detector(
    rpc: RpcClient,
    etherscan_key: Option<&str>,
) -> CompositeProxyDetector<AlchemyProxyDetector, CachedEtherscanProxyHint<EtherscanProxyHint>> {
    let primary = AlchemyProxyDetector::new(rpc);
    let hint = etherscan_key
        .and_then(|key| EtherscanClient::with_default_http(key.to_string()).ok())
        .map(EtherscanProxyHint::new)
        .map(CachedEtherscanProxyHint::new);
    CompositeProxyDetector::new(primary, hint)
}

fn build_token_search(etherscan_key: Option<&str>) -> TokenSearchBackend {
    etherscan_key
        .and_then(|key| EtherscanClient::with_default_http(key.to_string()).ok())
        .map(EtherscanTokenSearch::new)
        .map(TokenSearchBackend::Etherscan)
        .unwrap_or(TokenSearchBackend::Noop)
}

/// Composite ContractSourcePort used by the tx-detail pipeline so the
/// full enrichment (asset changes + state diff) always runs, even if
/// ETHERSCAN_API_KEY is not configured. Missing keys just mean no ABI
/// decoding.
#[derive(Clone)]
enum TxContractSource {
    Etherscan(EtherscanContractSource),
    Noop,
}

impl crate::application::ports::ContractSourcePort for TxContractSource {
    async fn get_abi(
        &self,
        address: crate::domain::Address,
        chain: Chain,
    ) -> Result<Option<crate::domain::ContractAbi>, crate::domain::DomainError> {
        match self {
            TxContractSource::Etherscan(inner) => inner.get_abi(address, chain).await,
            TxContractSource::Noop => Ok(None),
        }
    }

    async fn get_source(
        &self,
        address: crate::domain::Address,
        chain: Chain,
    ) -> Result<Option<crate::domain::ContractSource>, crate::domain::DomainError> {
        match self {
            TxContractSource::Etherscan(inner) => inner.get_source(address, chain).await,
            TxContractSource::Noop => Ok(None),
        }
    }
}

/// Same idea for the signature directory: when a concrete adapter
/// cannot be built (offline CI, unexpected URL parse failure, ...) we
/// fall back to a Noop and the UI shows the raw selector / topic0,
/// but the heavier tabs still populate.
#[derive(Clone)]
enum TxSignatureDir {
    /// openchain (primary) + Samczsun (fallback) wired behind a
    /// composite per `plan/15-backlog.md` section 3.2.
    Composite(CompositeSignatureDirectory<HttpSignatureDirectory, SamczsunSignatureDirectory>),
    /// openchain only, when the Samczsun adapter could not be built.
    Openchain(HttpSignatureDirectory),
    Noop,
}

impl crate::application::ports::SignatureDirectoryPort for TxSignatureDir {
    async fn lookup_selector(
        &self,
        selector: [u8; 4],
    ) -> Result<Option<crate::application::ports::SignatureHit>, crate::domain::DomainError> {
        match self {
            TxSignatureDir::Composite(inner) => inner.lookup_selector(selector).await,
            TxSignatureDir::Openchain(inner) => inner.lookup_selector(selector).await,
            TxSignatureDir::Noop => Ok(None),
        }
    }

    async fn lookup_event_topic(
        &self,
        topic: [u8; 32],
    ) -> Result<Option<crate::application::ports::SignatureHit>, crate::domain::DomainError> {
        match self {
            TxSignatureDir::Composite(inner) => inner.lookup_event_topic(topic).await,
            TxSignatureDir::Openchain(inner) => inner.lookup_event_topic(topic).await,
            TxSignatureDir::Noop => Ok(None),
        }
    }
}

/// Wire the fallback chain openchain → Samczsun, degrading to a
/// Noop when neither client can be built.
fn build_signature_directory() -> TxSignatureDir {
    let openchain = HttpSignatureDirectory::openchain_with_default_http().ok();
    let samczsun = SamczsunSignatureDirectory::with_default_http().ok();

    match (openchain, samczsun) {
        (Some(primary), Some(fallback)) => {
            TxSignatureDir::Composite(CompositeSignatureDirectory::new(primary, fallback))
        }
        (Some(primary), None) => TxSignatureDir::Openchain(primary),
        (None, _) => TxSignatureDir::Noop,
    }
}

pub use config::{ApiCredentials, AppConfig, ConfigLoader};

/// Hint printed when the binary is invoked without credentials and
/// without `--demo`.
const NO_DATA_HINT: &str = "blockexplorer-tui: no Alchemy key found.\n\
Set ALCHEMY_API_KEY and re-run, or launch with `cargo run -- --demo`\n\
to see the Home screen rendered with placeholder data.\n\
See plan/14-config-and-credentials.md for the full wiring.";

/// Entry point called from `main`.
pub fn run() -> Result<()> {
    // Install the masking logger before anything else so credentials
    // never hit stderr, even if config loading decides to log a
    // warning. See plan/10-settings.md §12.1.
    logging::install_masking_logger();

    let cli = parse_cli();
    let config = AppConfig::load().context("failed to load config")?;

    if cli.demo {
        // plan/10-settings.md §12.2: surface the first-run banner
        // when the demo binary is launched without a real key so the
        // user gets routed to Settings → Credentials before they try
        // to open a live screen.
        let first_run_hint = !config.has_alchemy_key();
        return boot_runtime(
            move |_| {
                let mut stack = ScreenStack::new();
                stack.push(Box::new(
                    HomeScreen::with_demo_data().with_first_run_hint(first_run_hint),
                ));
                stack
            },
            |_| default_keymap(),
        );
    }

    if !config.has_alchemy_key() {
        eprintln!("{NO_DATA_HINT}");
        return Ok(());
    }

    let config_for_keymap = config.clone();
    boot_runtime(
        move |_| build_live_stack(&config),
        move |_| build_live_keymap(&config_for_keymap),
    )
}

/// Thin wrapper that brings up the Tokio runtime and the event loop,
/// producing the initial [`ScreenStack`] and [`GlobalKeyMap`] via
/// `build_stack` and `build_keymap`.
fn boot_runtime<F, K>(build_stack: F, build_keymap: K) -> Result<()>
where
    F: FnOnce(&tokio::runtime::Handle) -> ScreenStack,
    K: FnOnce(&tokio::runtime::Handle) -> GlobalKeyMap,
{
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let handle = tokio::runtime::Handle::current();
        let stack = build_stack(&handle);
        let keymap = build_keymap(&handle);
        runtime::run_event_loop_with_keymap(stack, keymap).await
    })
}

/// Keymap used for the `--demo` boot path: only the help modal is
/// wired, because search depends on live adapters. See
/// `plan/12-screen-runtime.md` §7 item 5.
fn default_keymap() -> GlobalKeyMap {
    GlobalKeyMap::default().with_help_factory(demo_help_factory)
}

fn demo_help_factory() -> Box<dyn Screen> {
    Box::new(HelpModal::new(
        "blockexplorer-tui",
        vec![
            ("/".to_string(), "Open search".to_string()),
            ("?".to_string(), "Show help".to_string()),
            ("q".to_string(), "Quit".to_string()),
            ("Esc".to_string(), "Back / close modal".to_string()),
            ("Ctrl+C".to_string(), "Quit".to_string()),
        ],
    ))
}

/// Live keymap: wires the global `/` binding to a SearchScreen that
/// shares the live adapters, so every screen can open search as a
/// modal. `?` opens the help modal. See `plan/12-screen-runtime.md`
/// §7 item 5.
fn build_live_keymap(config: &AppConfig) -> GlobalKeyMap {
    let chain = config.chain;
    let key = match config.credentials.alchemy.as_deref() {
        Some(k) => k.to_string(),
        None => return default_keymap(),
    };
    let etherscan_key = config.credentials.etherscan.clone();

    let url = alchemy_url(chain, &key);
    let http = Client::new();
    let rpc = RpcClient::new(url, http);
    let search_cache: SearchCache = TtlCache::with_ttl(SEARCH_CACHE_TTL);

    let search_factory = {
        let rpc = rpc.clone();
        let etherscan_key = etherscan_key.clone();
        let alchemy_key = key.clone();
        move || -> Box<dyn Screen> {
            build_live_search_screen(
                chain,
                rpc.clone(),
                alchemy_key.clone(),
                etherscan_key.clone(),
                search_cache.clone(),
            )
        }
    };

    GlobalKeyMap::default()
        .with_search_factory(search_factory)
        .with_help_factory(demo_help_factory)
}

fn build_live_search_screen(
    chain: Chain,
    rpc: RpcClient,
    alchemy_key: String,
    etherscan_key: Option<String>,
    search_cache: SearchCache,
) -> Box<dyn Screen> {
    let block = AlchemyBlockLookup::new(rpc.clone());
    let tx = AlchemyTxLookup::new(rpc.clone());
    let addr = AlchemyAddressLookup::new(rpc.clone());
    let ens = AlchemyEnsResolver::new(rpc.clone());
    let token = build_token_search(etherscan_key.as_deref());
    let token_reader = AlchemyTokenReader::new(rpc.clone());
    let (search_feed_rx, sender) = search_feed();
    std::mem::drop(search_feed::spawn_with_cache(
        chain,
        block,
        tx,
        addr,
        ens,
        token,
        token_reader,
        sender,
        Some(search_cache),
    ));

    let detail_factory = {
        let rpc = rpc.clone();
        let etherscan_key = etherscan_key.clone();
        let alchemy_key = alchemy_key.clone();
        Box::new(move |entity: ResolvedEntity| -> Box<dyn Screen> {
            match entity {
                ResolvedEntity::Block { number, .. } => {
                    let reader = AlchemyBlockReader::new(rpc.clone());
                    let (feed, sender) = block_feed();
                    std::mem::drop(block_feed::spawn(chain, reader, sender));
                    let rpc_for_tx = rpc.clone();
                    let etherscan_for_tx = etherscan_key.clone();
                    let open_tx = Box::new(move |hash| {
                        live_tx_detail_screen(
                            chain,
                            hash,
                            rpc_for_tx.clone(),
                            etherscan_for_tx.clone(),
                        )
                    });
                    Box::new(BlockDetailScreen::loading(
                        chain,
                        BlockId::Number(number),
                        feed,
                        open_tx,
                    ))
                }
                ResolvedEntity::Tx { hash, .. } => {
                    live_tx_detail_screen(chain, hash, rpc.clone(), etherscan_key.clone())
                }
                ResolvedEntity::Address { address, .. } => live_address_detail_screen(
                    chain,
                    address,
                    rpc.clone(),
                    alchemy_key.clone(),
                    etherscan_key.clone(),
                ),
                ResolvedEntity::Contract { address } => {
                    live_contract_detail_screen(chain, address, rpc.clone(), etherscan_key.clone())
                }
                ResolvedEntity::Token(meta) => live_token_detail_screen(
                    chain,
                    meta.address,
                    rpc.clone(),
                    &alchemy_key,
                    etherscan_key.clone(),
                ),
                other => Box::new(DetailPlaceholderScreen::new(other)),
            }
        })
    };

    Box::new(SearchScreen::new(search_feed_rx, detail_factory))
}

fn build_live_stack(config: &AppConfig) -> ScreenStack {
    let chain = config.chain;
    let key = config
        .credentials
        .alchemy
        .as_deref()
        .expect("caller ensured the key is present");
    let etherscan_key = config.credentials.etherscan.clone();

    let url = alchemy_url(chain, key);
    let http = Client::new();
    let rpc = RpcClient::new(url, http);

    let network = AlchemyNetworkStatusAdapter::new(rpc.clone());
    let gas = AlchemyGasOracleAdapter::new(rpc.clone());
    let chains = InMemoryChainRegistry::with_default(chain);

    let session = HomeSession::new(network, gas, chains, chain);
    let (feed, _home_handle) = home_feed::start(session, home_feed::DEFAULT_REFRESH_PERIOD);

    let search_cache: SearchCache = TtlCache::with_ttl(SEARCH_CACHE_TTL);
    let search_factory = {
        let rpc = rpc.clone();
        let etherscan_key = etherscan_key.clone();
        let alchemy_key = key.to_string();
        let search_cache = search_cache.clone();
        Box::new(move || -> Box<dyn Screen> {
            let block = AlchemyBlockLookup::new(rpc.clone());
            let tx = AlchemyTxLookup::new(rpc.clone());
            let addr = AlchemyAddressLookup::new(rpc.clone());
            let ens = AlchemyEnsResolver::new(rpc.clone());
            let token = build_token_search(etherscan_key.as_deref());
            let token_reader = AlchemyTokenReader::new(rpc.clone());
            let (search_feed_rx, sender) = search_feed();
            std::mem::drop(search_feed::spawn_with_cache(
                chain,
                block,
                tx,
                addr,
                ens,
                token,
                token_reader,
                sender,
                Some(search_cache.clone()),
            ));

            let detail_factory = {
                let rpc = rpc.clone();
                let etherscan_key = etherscan_key.clone();
                let alchemy_key = alchemy_key.clone();
                Box::new(move |entity: ResolvedEntity| -> Box<dyn Screen> {
                    match entity {
                        ResolvedEntity::Block { number, .. } => {
                            let reader = AlchemyBlockReader::new(rpc.clone());
                            let (feed, sender) = block_feed();
                            std::mem::drop(block_feed::spawn(chain, reader, sender));
                            let rpc_for_tx = rpc.clone();
                            let etherscan_for_tx = etherscan_key.clone();
                            let open_tx = Box::new(move |hash| {
                                live_tx_detail_screen(
                                    chain,
                                    hash,
                                    rpc_for_tx.clone(),
                                    etherscan_for_tx.clone(),
                                )
                            });
                            Box::new(BlockDetailScreen::loading(
                                chain,
                                BlockId::Number(number),
                                feed,
                                open_tx,
                            ))
                        }
                        ResolvedEntity::Tx { hash, .. } => {
                            live_tx_detail_screen(chain, hash, rpc.clone(), etherscan_key.clone())
                        }
                        ResolvedEntity::Address { address, .. } => {
                            // Always route to AddressDetail: the screen
                            // itself detects bytecode and surfaces a
                            // Contract tab that drills into the
                            // ContractDetailScreen when the user asks
                            // for it. This way wallets and contracts
                            // share the same entry point and neither
                            // flavour loses balance / transfers /
                            // tokens.
                            live_address_detail_screen(
                                chain,
                                address,
                                rpc.clone(),
                                alchemy_key.clone(),
                                etherscan_key.clone(),
                            )
                        }
                        ResolvedEntity::Contract { address } => {
                            // Search-list shortcut: jump straight to
                            // the ContractDetailScreen when the user
                            // picked the "open as contract" row.
                            live_contract_detail_screen(
                                chain,
                                address,
                                rpc.clone(),
                                etherscan_key.clone(),
                            )
                        }
                        ResolvedEntity::Token(meta) => live_token_detail_screen(
                            chain,
                            meta.address,
                            rpc.clone(),
                            &alchemy_key,
                            etherscan_key.clone(),
                        ),
                        other => Box::new(DetailPlaceholderScreen::new(other)),
                    }
                })
            };

            Box::new(SearchScreen::new(search_feed_rx, detail_factory))
        })
    };

    // Mempool stays connected to an empty stream until the WS adapter
    // is wired end-to-end (plan/5 §11.3.4). Opening the screen works
    // and the "waiting..." empty state is rendered. The control
    // channel + status feed are wired unconditionally so the screen
    // exercises the same surface the live adapter will consume.
    let mempool_factory = {
        let rpc_for_tx = rpc.clone();
        let etherscan_for_tx = etherscan_key.clone();
        Box::new(move || -> Box<dyn Screen> {
            let stream = EmptyPendingTxStream;
            let rx = futures_block_on(async {
                stream
                    .subscribe(chain, PendingTxFilter::default())
                    .await
                    .expect("EmptyPendingTxStream cannot fail")
            });
            // Filter drain: forwards set_filter broadcasts to
            // EmptyPendingTxStream::update_filter (today a no-op;
            // tomorrow the live WS adapter will consume these).
            let (filter_control, _drain_handle) = spawn_filter_drain(stream, chain);

            let rpc = rpc_for_tx.clone();
            let etherscan_key = etherscan_for_tx.clone();
            let open_tx = Box::new(move |hash| {
                live_tx_detail_screen(chain, hash, rpc.clone(), etherscan_key.clone())
            });
            // TODO(plan/5 §11.3.4): when the live WS adapter lands,
            // plumb mempool_status_feed() in here and publish
            // Connected / Disconnected on the reconnect loop.
            Box::new(
                MempoolScreen::new(rx, PendingTxFilter::default(), open_tx)
                    .with_filter_control(filter_control),
            )
        })
    };

    let gas_factory = {
        let rpc = rpc.clone();
        Box::new(move || -> Box<dyn Screen> {
            let oracle = AlchemyGasOracleAdapter::new(rpc.clone());
            let (feed, sender) = gas_feed();
            // plan/9 §11.2: Ctrl+R in the Gas Tracker screen kicks
            // the listener end so the polling task skips the 6s
            // sleep and refetches immediately.
            let (refresh_handle, refresh_listener) = gas_refresh_channel();
            std::mem::drop(gas_feed::spawn_with_refresh(
                chain,
                oracle,
                sender,
                gas_feed::DEFAULT_REFRESH_PERIOD,
                Some(refresh_listener),
            ));
            Box::new(GasTrackerScreen::new(chain, None, feed).with_refresh_handle(refresh_handle))
        })
    };

    let settings_factory = {
        let snapshot = AppConfigSnapshot {
            chain,
            alchemy_key_present: config.has_alchemy_key(),
            config_path_hint: Some("~/.config/blockexplorer-tui/config.toml (via XDG)".to_string()),
        };
        Box::new(move || -> Box<dyn Screen> { Box::new(SettingsScreen::new(snapshot.clone())) })
    };

    let mut stack = ScreenStack::new();
    stack.push(Box::new(
        HomeScreen::with_feed(loading_view(chain), feed)
            .with_search_factory(search_factory)
            .with_mempool_factory(mempool_factory)
            .with_gas_factory(gas_factory)
            .with_settings_factory(settings_factory),
    ));
    stack
}

/// Block on a future from a non-async context by polling once. Used
/// only inside screen factories, which always return immediately
/// because the wrapped futures never suspend.
fn futures_block_on<F: std::future::Future>(fut: F) -> F::Output {
    use std::{
        pin::pin,
        task::{Context, Poll, Waker},
    };
    let mut fut = pin!(fut);
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    match fut.as_mut().poll(&mut cx) {
        Poll::Ready(value) => value,
        Poll::Pending => {
            panic!("screen factory future returned Pending; must be synchronous")
        }
    }
}

fn loading_view(chain: Chain) -> HomeViewModel {
    HomeViewModel {
        chain,
        network: None,
        gas: None,
        connection: ConnectionStatus::Connected,
    }
}

fn alchemy_url(chain: Chain, api_key: &str) -> Url {
    let raw = format!(
        "https://{subdomain}.g.alchemy.com/v2/{api_key}",
        subdomain = chain.alchemy_subdomain(),
    );
    Url::parse(&raw).expect("well-known Alchemy URL always parses")
}

#[derive(Debug, Default)]
struct Cli {
    demo: bool,
}

fn parse_cli() -> Cli {
    let mut cli = Cli::default();
    for arg in std::env::args().skip(1) {
        if arg == "--demo" {
            cli.demo = true;
        }
    }
    cli
}
