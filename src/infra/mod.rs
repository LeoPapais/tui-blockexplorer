//! Composition root.
//!
//! Parses the CLI, loads config, decides whether to run with real
//! Alchemy data or the frozen demo fixture, and finally starts the
//! event loop in a Tokio multi-thread runtime.
//!
//! See `plan/12-screen-runtime.md` (runtime) and
//! `plan/14-config-and-credentials.md` (credentials + wiring).

pub mod config;
mod home_feed;
mod runtime;

use anyhow::{Context, Result};
use reqwest::Client;
use url::Url;

use crate::{
    adapters::{
        config::InMemoryChainRegistry,
        rpc::{AlchemyGasOracleAdapter, AlchemyNetworkStatusAdapter, RpcClient},
        ui::{HomeScreen, ScreenStack},
    },
    application::{ConnectionStatus, HomeSession, HomeViewModel},
    domain::Chain,
};

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
    let gas = AlchemyGasOracleAdapter::new(rpc);
    let chains = InMemoryChainRegistry::with_default(chain);

    let session = HomeSession::new(network, gas, chains, chain);
    let (feed, _handle) = home_feed::start(session, home_feed::DEFAULT_REFRESH_PERIOD);

    let mut stack = ScreenStack::new();
    stack.push(Box::new(HomeScreen::with_feed(loading_view(chain), feed)));
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
