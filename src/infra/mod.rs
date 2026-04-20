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
pub mod cost_meter;
mod gas_feed;
pub mod health;
pub mod home_feed;
pub mod logging;
pub mod mempool_feed;
pub mod navigate;
pub mod runtime;
pub mod search_feed;
mod tx_feed;

use anyhow::{Context, Result};
use reqwest::Client;
use url::Url;

use std::sync::Arc;
use std::time::Duration;

use cost_meter::CostMeter;

use crate::{
    adapters::{
        cache::TtlCache,
        clock::SystemClock,
        config::InMemoryChainRegistry,
        etherscan::{
            CachedEtherscanProxyHint, EtherscanClient, EtherscanContractSource, EtherscanProxyHint,
            EtherscanTokenSearch,
        },
        prices::{AlchemyPrices, PollingTokenPriceStream, PricesClient},
        rng::OsRng,
        rpc::{
            AlchemyAddressLookup, AlchemyAddressReader, AlchemyBlockLookup, AlchemyBlockReader,
            AlchemyContractReader, AlchemyEnsResolver, AlchemyEventLog, AlchemyGasOracleAdapter,
            AlchemyNetworkStatusAdapter, AlchemyPortfolio, AlchemyProxyDetector, AlchemySimulation,
            AlchemyStorage, AlchemyTokenReader, AlchemyTransfers, AlchemyTxLookup, AlchemyTxReader,
            AlchemyTxTracer, CircuitBreaker, CompositeProxyDetector, RpcClient,
        },
        signatures::{
            CompositeSignatureDirectory, HttpSignatureDirectory, SamczsunSignatureDirectory,
        },
        ui::{
            AddressDetailScreen, AddressTab, AppConfigSnapshot, BlockDetailScreen, CursorServices,
            DetailPlaceholderScreen, GasTrackerScreen, GlobalKeyMap, HelpModal, HomeScreen,
            MempoolScreen, Screen, ScreenStack, SearchScreen, SettingsScreen, TxDetailScreen,
            address_feed, block_feed, gas_feed, gas_refresh_channel, search_feed, tx_feed,
        },
    },
    application::{ConnectionStatus, HomeSession, HomeViewModel, ports::PendingTxStreamPort},
    domain::{BlockId, Chain, PendingTxFilter, ResolvedEntity},
    infra::search_feed::SearchCache,
};
use mempool_feed::{EmptyPendingTxStream, spawn_filter_drain};

/// Per-(chain, input) TTL for the search resolution cache. Pulled
/// from `adapters::cache::SEARCH_TTL` so the composition root and the
/// rest of the `CacheRegistry` consumers agree on a single number.
/// Documented in `plan/2-search.md` section 12.4 and
/// `plan/15-backlog.md` §8.16 (Cache registry).
const SEARCH_CACHE_TTL: Duration = crate::adapters::cache::SEARCH_TTL;

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
    cursor_services: Option<CursorServices>,
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

    let screen = TxDetailScreen::loading(chain, hash, feed);
    match cursor_services {
        Some(services) => Box::new(screen.with_cursor_services(services)),
        None => Box::new(screen),
    }
}

/// Build a live unified `AddressDetailScreen`.
///
/// The screen absorbs what used to live in the separate Contract
/// and Token detail screens. Callers pass an `initial_tab` hint so
/// search results for `ResolvedEntity::Contract` / `Token` open the
/// screen already focused on the right main tab (see
/// `plan/16-unified-address-detail.md` §7).
fn live_address_detail_screen(
    chain: Chain,
    address: crate::domain::Address,
    initial_tab: AddressTab,
    rpc: RpcClient,
    alchemy_key: String,
    etherscan_key: Option<String>,
    cursor_services: Option<CursorServices>,
) -> Box<dyn Screen> {
    let reader = AlchemyAddressReader::new(rpc.clone());
    let transfers = AlchemyTransfers::new(rpc.clone());
    let portfolio = AlchemyPortfolio::new(rpc.clone());
    let token_reader = AlchemyTokenReader::new(rpc.clone());
    let prices = match PricesClient::with_api_key(&alchemy_key) {
        Ok(client) => TokenPrices::Alchemy(AlchemyPrices::new(client)),
        Err(_) => TokenPrices::Noop,
    };
    let ens = crate::adapters::ens::CachedEnsResolver::new(AlchemyEnsResolver::new(rpc.clone()));
    let proxy_detector = build_proxy_detector(rpc.clone(), etherscan_key.as_deref());
    let contract_source = etherscan_key
        .clone()
        .and_then(|key| EtherscanClient::with_default_http(key).ok())
        .map(EtherscanContractSource::new)
        .map(TxContractSource::Etherscan)
        .unwrap_or(TxContractSource::Noop);
    let contract_reader = AlchemyContractReader::new(rpc.clone());
    let event_log = AlchemyEventLog::new(rpc.clone());
    let storage = AlchemyStorage::new(rpc.clone());
    let network_status = AlchemyNetworkStatusAdapter::new(rpc.clone());
    let price_stream = PollingTokenPriceStream::with_default_interval(prices.clone());

    let (feed, sender) = address_feed();
    std::mem::drop(address_feed::spawn(
        chain,
        reader,
        transfers,
        portfolio,
        token_reader,
        prices,
        ens,
        proxy_detector,
        contract_source,
        contract_reader,
        event_log,
        storage,
        network_status,
        price_stream,
        sender,
    ));

    let rpc_for_tx = rpc.clone();
    let etherscan_for_tx = etherscan_key.clone();
    let services_for_tx = cursor_services.clone();
    let open_tx: crate::adapters::ui::address_detail::OpenTxFactory = Box::new(move |hash| {
        live_tx_detail_screen(
            chain,
            hash,
            rpc_for_tx.clone(),
            etherscan_for_tx.clone(),
            services_for_tx.clone(),
        )
    });

    let rpc_for_token = rpc;
    let alchemy_for_token = alchemy_key;
    let etherscan_for_token = etherscan_key;
    let services_for_token = cursor_services.clone();
    let open_token: crate::adapters::ui::address_detail::OpenTokenFactory =
        Box::new(move |contract| {
            live_address_detail_screen(
                chain,
                contract,
                AddressTab::Token,
                rpc_for_token.clone(),
                alchemy_for_token.clone(),
                etherscan_for_token.clone(),
                services_for_token.clone(),
            )
        });

    let screen = AddressDetailScreen::with_factories_and_tab(
        chain,
        address,
        feed,
        Some(open_tx),
        Some(open_token),
        initial_tab,
    );
    match cursor_services {
        Some(services) => Box::new(screen.with_cursor_services(services)),
        None => Box::new(screen),
    }
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

/// Resolve the default XDG config path (same logic the loader uses)
/// so callers can mention it in error messages before the user has a
/// file on disk. Returns `None` when the `directories` crate cannot
/// produce a project directory for this OS (shouldn't happen on any
/// supported target).
#[must_use]
pub fn default_config_path() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("", "", "blockexplorer-tui")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

/// Seed a `config.toml` at `path` with default values, suitable for
/// the user to edit afterwards. The write goes through the atomic
/// `FsConfig::save` so we reuse the temp-file-and-rename contract
/// `plan/10-settings.md` §12.3 already ships.
///
/// If the file already exists its contents are preserved: existing
/// credentials and a user-picked default chain survive. This keeps
/// `cargo run -- --init-config` idempotent.
///
/// See `plan/14-config-and-credentials.md` §8.3.
pub fn init_config_at(
    path: &std::path::Path,
) -> std::result::Result<crate::application::ports::AppConfigView, crate::domain::DomainError> {
    use crate::{
        adapters::config::FsConfig,
        application::ports::{ConfigPatch, ConfigPort},
    };

    let adapter = FsConfig::new(path.to_path_buf());

    // `FsConfig` is implemented on top of `std::fs`, so the futures
    // returned by `load` / `save` complete without yielding. We can
    // therefore synchronously block on them from a non-async CLI
    // context via the same helper used by the screen factories.
    let existing = futures_block_on(adapter.load())?;

    // Either the file is brand new (load returns Chain::Ethereum by
    // default) or the user already picked something; either way
    // materialising the current `[defaults]` block via `save`
    // ensures the next boot reads a self-contained seed.
    let patch = ConfigPatch::with_default_chain(existing.chain);

    futures_block_on(adapter.save(&patch))
}

/// Hint printed when the binary is invoked without credentials and
/// without `--demo`. The exact text is asserted by integration tests
/// via `NO_DATA_HINT_TEMPLATE`.
fn no_data_hint(config_path: Option<&std::path::Path>) -> String {
    let path_line = match config_path {
        Some(p) => format!("  {}\n", p.display()),
        None => String::from("  (config path unavailable on this OS)\n"),
    };
    format!(
        "blockexplorer-tui: no Alchemy key found.\n\
Set ALCHEMY_API_KEY and re-run, or launch with `cargo run -- --demo`\n\
to see the Home screen rendered with placeholder data.\n\
\n\
A seed config file can be created at:\n\
{path_line}\
by running `cargo run -- --init-config`; edit the file afterwards\n\
to persist your Alchemy / Etherscan keys.\n\
\n\
See plan/14-config-and-credentials.md for the full wiring."
    )
}

/// Entry point called from `main`.
pub fn run() -> Result<()> {
    // Install the masking logger before anything else so credentials
    // never hit stderr, even if config loading decides to log a
    // warning. See plan/10-settings.md §12.1.
    logging::install_masking_logger();

    let cli = parse_cli();

    // `--init-config` is handled before config loading so the flag
    // still works when the file does not exist (that is, in fact,
    // the only case where the flag is useful). Config parsing errors
    // on the existing file would otherwise mask the bootstrap flow.
    if cli.init_config {
        let Some(path) = default_config_path() else {
            anyhow::bail!(
                "could not resolve XDG config path for this OS; \
                pass a path via `--init-config=<path>` once supported"
            );
        };
        let view = init_config_at(&path).context("failed to seed config")?;
        eprintln!(
            "blockexplorer-tui: wrote seed config to {}\n\
             default chain: {}\n\
             edit the file to add your Alchemy / Etherscan keys.",
            path.display(),
            view.chain.slug()
        );
        return Ok(());
    }

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
        eprintln!("{}", no_data_hint(default_config_path().as_deref()));
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
    Box::new(HelpModal::new("blockexplorer-tui", help_entries()))
}

/// Canonical help-modal entries, shared by the demo and live
/// keymaps so the `?` overlay stays consistent across boot paths.
/// New rows must be APPENDED (never reordered) because some
/// functional tests assert on the top-of-list rendering.
fn help_entries() -> Vec<(String, String)> {
    vec![
        ("/".to_string(), "Open search".to_string()),
        ("?".to_string(), "Show help".to_string()),
        ("q".to_string(), "Quit".to_string()),
        ("Esc".to_string(), "Back / close modal".to_string()),
        ("Ctrl+C".to_string(), "Quit".to_string()),
        ("Tab / Shift-Tab".to_string(), "Switch main tab".to_string()),
        (
            "[ / ]".to_string(),
            "Switch sub-tab (Contract/Token)".to_string(),
        ),
        ("1..9".to_string(), "Jump directly to sub-tab".to_string()),
        ("Arrows".to_string(), "Move field cursor".to_string()),
        (
            "Enter".to_string(),
            "Open related screen for field under cursor".to_string(),
        ),
        (
            "y".to_string(),
            "Copy field under cursor (or screen's default value)".to_string(),
        ),
        (
            "Y".to_string(),
            "Copy ENS / canonical identifier".to_string(),
        ),
        (
            "e".to_string(),
            "Export active tab as CSV (Address/Block)".to_string(),
        ),
    ]
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
    let breaker = Arc::new(CircuitBreaker::default_with_clock(Arc::new(
        SystemClock::new(),
    )));
    let rpc = RpcClient::new(url, http)
        .with_default_retry(Arc::new(OsRng::new()))
        .with_circuit_breaker(breaker)
        .with_cost_recorder(CostMeter::shared());
    let search_cache: SearchCache = TtlCache::with_ttl(SEARCH_CACHE_TTL);

    // The keymap's search modal is an independent screen stack (it
    // lives in the modal slot), so it deserves its own
    // `CursorServices`. `live_cursor_services` acquires a fresh
    // `ArboardClipboard` handle and logs if the host has no display.
    let cursor_services =
        navigate::live_cursor_services(rpc.clone(), key.clone(), etherscan_key.clone(), chain);

    let search_factory = {
        let rpc = rpc.clone();
        let etherscan_key = etherscan_key.clone();
        let alchemy_key = key.clone();
        let cursor_services = cursor_services.clone();
        move || -> Box<dyn Screen> {
            build_live_search_screen(
                chain,
                rpc.clone(),
                alchemy_key.clone(),
                etherscan_key.clone(),
                search_cache.clone(),
                cursor_services.clone(),
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
    cursor_services: CursorServices,
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
        let cursor_services = cursor_services.clone();
        Box::new(move |entity: ResolvedEntity| -> Box<dyn Screen> {
            match entity {
                ResolvedEntity::Block { number, .. } => {
                    let reader = AlchemyBlockReader::new(rpc.clone());
                    let (feed, sender) = block_feed();
                    std::mem::drop(block_feed::spawn(chain, reader, sender));
                    let rpc_for_tx = rpc.clone();
                    let etherscan_for_tx = etherscan_key.clone();
                    let services_for_tx = cursor_services.clone();
                    let open_tx = Box::new(move |hash| {
                        live_tx_detail_screen(
                            chain,
                            hash,
                            rpc_for_tx.clone(),
                            etherscan_for_tx.clone(),
                            Some(services_for_tx.clone()),
                        )
                    });
                    Box::new(
                        BlockDetailScreen::loading(chain, BlockId::Number(number), feed, open_tx)
                            .with_cursor_services(cursor_services.clone()),
                    )
                }
                ResolvedEntity::Tx { hash, .. } => live_tx_detail_screen(
                    chain,
                    hash,
                    rpc.clone(),
                    etherscan_key.clone(),
                    Some(cursor_services.clone()),
                ),
                ResolvedEntity::Address { address, .. }
                | ResolvedEntity::DelegatedEoa { address, .. } => live_address_detail_screen(
                    chain,
                    address,
                    AddressTab::Overview,
                    rpc.clone(),
                    alchemy_key.clone(),
                    etherscan_key.clone(),
                    Some(cursor_services.clone()),
                ),
                ResolvedEntity::Contract { address } => live_address_detail_screen(
                    chain,
                    address,
                    AddressTab::Contract,
                    rpc.clone(),
                    alchemy_key.clone(),
                    etherscan_key.clone(),
                    Some(cursor_services.clone()),
                ),
                ResolvedEntity::Token(meta) => live_address_detail_screen(
                    chain,
                    meta.address,
                    AddressTab::Token,
                    rpc.clone(),
                    alchemy_key.clone(),
                    etherscan_key.clone(),
                    Some(cursor_services.clone()),
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
    let breaker = Arc::new(CircuitBreaker::default_with_clock(Arc::new(
        SystemClock::new(),
    )));
    let rpc = RpcClient::new(url, http)
        .with_default_retry(Arc::new(OsRng::new()))
        .with_circuit_breaker(breaker)
        .with_cost_recorder(CostMeter::shared());

    let network = AlchemyNetworkStatusAdapter::new(rpc.clone());
    let gas = AlchemyGasOracleAdapter::new(rpc.clone());
    let chains = InMemoryChainRegistry::with_default(chain);

    let session = HomeSession::new(network, gas, chains, chain);
    let (feed, _home_handle) = home_feed::start(session, home_feed::DEFAULT_REFRESH_PERIOD);

    // Build the `CursorServices` bundle ONCE per live stack so every
    // screen factory (Home, Mempool, Gas, Settings, and the detail
    // screens opened via search) shares the same clipboard handle.
    // See `plan/17-navigable-values.md` §5.
    let cursor_services =
        navigate::live_cursor_services(rpc.clone(), key.to_string(), etherscan_key.clone(), chain);

    let search_cache: SearchCache = TtlCache::with_ttl(SEARCH_CACHE_TTL);
    let search_factory = {
        let rpc = rpc.clone();
        let etherscan_key = etherscan_key.clone();
        let alchemy_key = key.to_string();
        let search_cache = search_cache.clone();
        let cursor_services = cursor_services.clone();
        Box::new(move || -> Box<dyn Screen> {
            build_live_search_screen(
                chain,
                rpc.clone(),
                alchemy_key.clone(),
                etherscan_key.clone(),
                search_cache.clone(),
                cursor_services.clone(),
            )
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
        let cursor_services = cursor_services.clone();
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
            let services_for_tx = cursor_services.clone();
            let open_tx = Box::new(move |hash| {
                live_tx_detail_screen(
                    chain,
                    hash,
                    rpc.clone(),
                    etherscan_key.clone(),
                    Some(services_for_tx.clone()),
                )
            });
            // TODO(plan/5 §11.3.4): when the live WS adapter lands,
            // plumb mempool_status_feed() in here and publish
            // Connected / Disconnected on the reconnect loop.
            Box::new(
                MempoolScreen::new(rx, PendingTxFilter::default(), open_tx)
                    .with_filter_control(filter_control)
                    .with_cursor_services(cursor_services.clone()),
            )
        })
    };

    let gas_factory = {
        let rpc = rpc.clone();
        let cursor_services = cursor_services.clone();
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
            Box::new(
                GasTrackerScreen::new(chain, None, feed)
                    .with_refresh_handle(refresh_handle)
                    .with_cursor_services(cursor_services.clone()),
            )
        })
    };

    let settings_factory = {
        let snapshot = AppConfigSnapshot {
            chain,
            alchemy_key_present: config.has_alchemy_key(),
            config_path_hint: Some("~/.config/blockexplorer-tui/config.toml (via XDG)".to_string()),
        };
        let cursor_services = cursor_services.clone();
        Box::new(move || -> Box<dyn Screen> {
            Box::new(
                SettingsScreen::new(snapshot.clone()).with_cursor_services(cursor_services.clone()),
            )
        })
    };

    let mut stack = ScreenStack::new();
    stack.push(Box::new(
        HomeScreen::with_feed(loading_view(chain), feed)
            .with_search_factory(search_factory)
            .with_mempool_factory(mempool_factory)
            .with_gas_factory(gas_factory)
            .with_settings_factory(settings_factory)
            .with_cursor_services(cursor_services),
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
    init_config: bool,
}

fn parse_cli() -> Cli {
    let mut cli = Cli::default();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--demo" => cli.demo = true,
            "--init-config" => cli.init_config = true,
            _ => {}
        }
    }
    cli
}
