//! Composition root.
//!
//! Parses the CLI, loads config, decides whether to run with real
//! Alchemy data or the frozen demo fixture, and finally starts the
//! event loop in a Tokio multi-thread runtime.
//!
//! See `plan/12-screen-runtime.md` (runtime) and
//! `plan/14-config-and-credentials.md` (credentials + wiring).

mod block_feed;
pub mod config;
mod home_feed;
mod runtime;
mod search_feed;
mod tx_feed;

use anyhow::{Context, Result};
use reqwest::Client;
use url::Url;

use crate::{
    adapters::{
        config::InMemoryChainRegistry,
        rpc::{
            AlchemyAddressLookup, AlchemyBlockLookup, AlchemyBlockReader, AlchemyEnsResolver,
            AlchemyGasOracleAdapter, AlchemyNetworkStatusAdapter, AlchemyTxLookup,
            AlchemyTxReader, RpcClient,
        },
        ui::{
            BlockDetailScreen, DetailPlaceholderScreen, HomeScreen, Screen, ScreenStack,
            SearchScreen, TxDetailScreen, block_feed, search_feed, tx_feed,
        },
    },
    application::{ConnectionStatus, HomeSession, HomeViewModel},
    domain::{BlockId, Chain, ResolvedEntity},
};

/// Build a live `TxDetailScreen` backed by a dedicated Alchemy
/// tx-reader task.
fn live_tx_detail_screen(
    chain: Chain,
    hash: crate::domain::TxHash,
    rpc: RpcClient,
) -> Box<dyn Screen> {
    let reader = AlchemyTxReader::new(rpc);
    let (feed, sender) = tx_feed();
    std::mem::drop(tx_feed::spawn(chain, reader, sender));
    Box::new(TxDetailScreen::loading(chain, hash, feed))
}

/// Stub token search used until the Etherscan adapter lands. Returns
/// no candidates for every query.
#[derive(Default, Clone, Copy)]
struct NoopTokenSearch;

impl crate::application::ports::TokenSearchPort for NoopTokenSearch {
    async fn by_symbol(
        &self,
        _symbol: &str,
        _chain: Chain,
    ) -> Result<Vec<crate::domain::TokenMetadata>, crate::domain::DomainError> {
        Ok(Vec::new())
    }

    async fn by_name(
        &self,
        _text: &str,
        _chain: Chain,
    ) -> Result<Vec<crate::domain::TokenMetadata>, crate::domain::DomainError> {
        Ok(Vec::new())
    }
}

pub use config::{AppConfig, ApiCredentials, ConfigLoader};

/// Hint printed when the binary is invoked without credentials and
/// without `--demo`.
const NO_DATA_HINT: &str = "blockexplorer-tui: no Alchemy key found.\n\
Set ALCHEMY_API_KEY and re-run, or launch with `cargo run -- --demo`\n\
to see the Home screen rendered with placeholder data.\n\
See plan/14-config-and-credentials.md for the full wiring.";

/// Entry point called from `main`.
pub fn run() -> Result<()> {
    let cli = parse_cli();
    let config = AppConfig::load().context("failed to load config")?;

    if cli.demo {
        return boot_runtime(|_| {
            let mut stack = ScreenStack::new();
            stack.push(Box::new(HomeScreen::with_demo_data()));
            stack
        });
    }

    if !config.has_alchemy_key() {
        eprintln!("{NO_DATA_HINT}");
        return Ok(());
    }

    boot_runtime(move |_| build_live_stack(&config))
}

/// Thin wrapper that brings up the Tokio runtime and the event loop,
/// producing the initial [`ScreenStack`] via `build`.
fn boot_runtime<F>(build: F) -> Result<()>
where
    F: FnOnce(&tokio::runtime::Handle) -> ScreenStack,
{
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let handle = tokio::runtime::Handle::current();
        let stack = build(&handle);
        runtime::run_event_loop(stack).await
    })
}

fn build_live_stack(config: &AppConfig) -> ScreenStack {
    let chain = config.chain;
    let key = config
        .credentials
        .alchemy
        .as_deref()
        .expect("caller ensured the key is present");

    let url = alchemy_url(chain, key);
    let http = Client::new();
    let rpc = RpcClient::new(url, http);

    let network = AlchemyNetworkStatusAdapter::new(rpc.clone());
    let gas = AlchemyGasOracleAdapter::new(rpc.clone());
    let chains = InMemoryChainRegistry::with_default(chain);

    let session = HomeSession::new(network, gas, chains, chain);
    let (feed, _home_handle) = home_feed::start(session, home_feed::DEFAULT_REFRESH_PERIOD);

    let search_factory = {
        let rpc = rpc.clone();
        Box::new(move || -> Box<dyn Screen> {
            let block = AlchemyBlockLookup::new(rpc.clone());
            let tx = AlchemyTxLookup::new(rpc.clone());
            let addr = AlchemyAddressLookup::new(rpc.clone());
            let ens = AlchemyEnsResolver::new(rpc.clone());
            let token = NoopTokenSearch;
            let (search_feed_rx, sender) = search_feed();
            // JoinHandle intentionally dropped: the task lives for as
            // long as the receiver end is alive, which matches the
            // lifetime of the SearchScreen we return.
            std::mem::drop(search_feed::spawn(
                chain, block, tx, addr, ens, token, sender,
            ));

            let detail_factory = {
                let rpc = rpc.clone();
                Box::new(move |entity: ResolvedEntity| -> Box<dyn Screen> {
                    match entity {
                        ResolvedEntity::Block { number, .. } => {
                            let reader = AlchemyBlockReader::new(rpc.clone());
                            let (feed, sender) = block_feed();
                            std::mem::drop(block_feed::spawn(chain, reader, sender));
                            // When the user drills into a tx from the
                            // block, wire a fresh TxDetail screen backed
                            // by its own tx-reader task.
                            let rpc_for_tx = rpc.clone();
                            let open_tx = Box::new(move |hash| {
                                live_tx_detail_screen(chain, hash, rpc_for_tx.clone())
                            });
                            Box::new(BlockDetailScreen::loading(
                                chain,
                                BlockId::Number(number),
                                feed,
                                open_tx,
                            ))
                        }
                        ResolvedEntity::Tx { hash, .. } => {
                            live_tx_detail_screen(chain, hash, rpc.clone())
                        }
                        other => Box::new(DetailPlaceholderScreen::new(other)),
                    }
                })
            };

            Box::new(SearchScreen::new(search_feed_rx, detail_factory))
        })
    };

    let mut stack = ScreenStack::new();
    stack.push(Box::new(
        HomeScreen::with_feed(loading_view(chain), feed).with_search_factory(search_factory),
    ));
    stack
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
