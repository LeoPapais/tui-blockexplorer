//! Composition root.
//!
//! Parses the CLI, loads config, decides whether to run with real
//! Alchemy data or the frozen demo fixture, and finally starts the
//! event loop in a Tokio multi-thread runtime.
//!
//! See `plan/12-screen-runtime.md` (runtime) and
//! `plan/14-config-and-credentials.md` (credentials + wiring).

mod address_feed;
mod block_feed;
pub mod config;
mod contract_feed;
mod gas_feed;
mod home_feed;
mod mempool_feed;
mod runtime;
mod search_feed;
mod token_feed;
mod tx_feed;

use anyhow::{Context, Result};
use reqwest::Client;
use url::Url;

use crate::{
    adapters::{
        config::InMemoryChainRegistry,
        rpc::{
            AlchemyAddressLookup, AlchemyAddressReader, AlchemyBlockLookup,
            AlchemyBlockReader, AlchemyEnsResolver, AlchemyGasOracleAdapter,
            AlchemyNetworkStatusAdapter, AlchemyProxyDetector, AlchemyTokenReader,
            AlchemyTxLookup, AlchemyTxReader, RpcClient,
        },
        ui::{
            AddressDetailScreen, BlockDetailScreen, ContractDetailScreen,
            DetailPlaceholderScreen, GasTrackerScreen, HomeScreen, MempoolScreen, Screen,
            ScreenStack, SearchScreen, TokenDetailScreen, TxDetailScreen, address_feed,
            block_feed, contract_feed, gas_feed, search_feed, token_feed, tx_feed,
        },
    },
    application::{
        ConnectionStatus, HomeSession, HomeViewModel, ports::PendingTxStreamPort,
    },
    domain::{AddressKind, BlockId, Chain, PendingTxFilter, ResolvedEntity},
};
use mempool_feed::EmptyPendingTxStream;

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

/// Build a live `AddressDetailScreen` backed by a dedicated
/// Alchemy address-reader task.
fn live_address_detail_screen(
    chain: Chain,
    address: crate::domain::Address,
    rpc: RpcClient,
) -> Box<dyn Screen> {
    let reader = AlchemyAddressReader::new(rpc);
    let (feed, sender) = address_feed();
    std::mem::drop(address_feed::spawn(chain, reader, sender));
    Box::new(AddressDetailScreen::loading(chain, address, feed))
}

/// Build a live `ContractDetailScreen` backed by address-reader +
/// proxy-detection tasks.
fn live_contract_detail_screen(
    chain: Chain,
    address: crate::domain::Address,
    rpc: RpcClient,
) -> Box<dyn Screen> {
    let reader = AlchemyAddressReader::new(rpc.clone());
    let detector = AlchemyProxyDetector::new(rpc);
    let (feed, sender) = contract_feed();
    std::mem::drop(contract_feed::spawn(chain, reader, detector, sender));
    Box::new(ContractDetailScreen::loading(chain, address, feed))
}

/// Build a live `TokenDetailScreen` backed by a dedicated
/// token-reader task.
fn live_token_detail_screen(
    chain: Chain,
    address: crate::domain::Address,
    rpc: RpcClient,
) -> Box<dyn Screen> {
    let reader = AlchemyTokenReader::new(rpc);
    let (feed, sender) = token_feed();
    std::mem::drop(token_feed::spawn(chain, reader, sender));
    Box::new(TokenDetailScreen::loading(chain, address, feed))
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
                        ResolvedEntity::Address { address, kind, .. } => match kind {
                            AddressKind::Contract => {
                                live_contract_detail_screen(chain, address, rpc.clone())
                            }
                            AddressKind::Eoa => {
                                live_address_detail_screen(chain, address, rpc.clone())
                            }
                        },
                        ResolvedEntity::Token(meta) => {
                            live_token_detail_screen(chain, meta.address, rpc.clone())
                        }
                        other => Box::new(DetailPlaceholderScreen::new(other)),
                    }
                })
            };

            Box::new(SearchScreen::new(search_feed_rx, detail_factory))
        })
    };

    // Mempool stays connected to an empty stream until the WS adapter
    // lands (plan/5 section 11.3). Opening the screen works; it just
    // renders the "waiting..." empty state.
    let mempool_factory = {
        let rpc_for_tx = rpc.clone();
        Box::new(move || -> Box<dyn Screen> {
            let stream = EmptyPendingTxStream;
            let rx = futures_block_on(async {
                stream
                    .subscribe(chain, PendingTxFilter::default())
                    .await
                    .expect("EmptyPendingTxStream cannot fail")
            });
            let rpc = rpc_for_tx.clone();
            let open_tx = Box::new(move |hash| live_tx_detail_screen(chain, hash, rpc.clone()));
            Box::new(MempoolScreen::new(
                rx,
                PendingTxFilter::default(),
                open_tx,
            ))
        })
    };

    let gas_factory = {
        let rpc = rpc.clone();
        Box::new(move || -> Box<dyn Screen> {
            let oracle = AlchemyGasOracleAdapter::new(rpc.clone());
            let (feed, sender) = gas_feed();
            std::mem::drop(gas_feed::spawn(
                chain,
                oracle,
                sender,
                gas_feed::DEFAULT_REFRESH_PERIOD,
            ));
            Box::new(GasTrackerScreen::new(chain, None, feed))
        })
    };

    let mut stack = ScreenStack::new();
    stack.push(Box::new(
        HomeScreen::with_feed(loading_view(chain), feed)
            .with_search_factory(search_factory)
            .with_mempool_factory(mempool_factory)
            .with_gas_factory(gas_factory),
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
