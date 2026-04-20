//! Live `NavigationFactory` implementation.
//!
//! Wires a [`NavigableValue`] to a boxed [`Screen`] using the same
//! `live_*_screen` helpers consumed by the search router. See
//! `plan/17-navigable-values.md` §5.3.

use std::sync::Arc;

use crate::{
    adapters::{
        rpc::RpcClient,
        ui::{
            AddressTab, BlockDetailScreen, NavigationFactory, Screen, block_feed,
            field_cursor::CursorServices,
        },
    },
    application::ports::ClipboardPort,
    domain::{BlockId, Chain, NavigableValue},
};

/// Live factory that opens Address / Block / Tx detail screens in
/// response to `Enter` on a navigable value. Uses a late-bound
/// `CursorServices` so the screen it opens carries the same
/// clipboard + navigation handle the parent did.
pub struct LiveNavigationFactory {
    rpc: RpcClient,
    alchemy_key: String,
    etherscan_key: Option<String>,
    services: std::sync::OnceLock<CursorServices>,
}

impl LiveNavigationFactory {
    #[must_use]
    pub fn new(rpc: RpcClient, alchemy_key: String, etherscan_key: Option<String>) -> Self {
        Self {
            rpc,
            alchemy_key,
            etherscan_key,
            services: std::sync::OnceLock::new(),
        }
    }

    /// Build an `Arc<dyn NavigationFactory>` so a single factory can
    /// be cloned into every screen's [`CursorServices`] bundle.
    #[must_use]
    pub fn arc(self) -> Arc<dyn NavigationFactory> {
        Arc::new(self)
    }

    /// Latch the shared `CursorServices` so the next screen this
    /// factory opens also carries the live clipboard handle. Called
    /// exactly once per live stack, right after the services are
    /// built. `OnceLock` keeps the factory `Send + Sync` without
    /// reaching for a `Mutex`.
    pub fn bind_services(&self, services: CursorServices) {
        let _ = self.services.set(services);
    }

    fn services(&self) -> Option<CursorServices> {
        self.services.get().cloned()
    }
}

impl NavigationFactory for LiveNavigationFactory {
    fn open(&self, value: &NavigableValue, chain: Chain) -> Option<Box<dyn Screen>> {
        let services = self.services();
        match value {
            NavigableValue::Plain(_) => None,
            NavigableValue::Address(addr) => Some(super::live_address_detail_screen(
                chain,
                *addr,
                AddressTab::Overview,
                self.rpc.clone(),
                self.alchemy_key.clone(),
                self.etherscan_key.clone(),
                services,
            )),
            // ENS forward-resolution from the cursor is deferred
            // (plan/15-backlog.md §3.1). Until that lands an
            // EnsName-only hit returns `None`; callers pair it with
            // an Address field in the same list.
            NavigableValue::EnsName(_) => None,
            NavigableValue::TokenAddress(addr) => Some(super::live_address_detail_screen(
                chain,
                *addr,
                AddressTab::Token,
                self.rpc.clone(),
                self.alchemy_key.clone(),
                self.etherscan_key.clone(),
                services,
            )),
            NavigableValue::TxHash(hash) => Some(super::live_tx_detail_screen(
                chain,
                *hash,
                self.rpc.clone(),
                self.etherscan_key.clone(),
                services,
            )),
            NavigableValue::BlockNumber(number) => {
                let reader = crate::adapters::rpc::AlchemyBlockReader::new(self.rpc.clone());
                let (feed, sender) = block_feed();
                std::mem::drop(super::block_feed::spawn(chain, reader, sender));
                let rpc = self.rpc.clone();
                let etherscan_key = self.etherscan_key.clone();
                let tx_services = services.clone();
                let open_tx = Box::new(move |hash| {
                    super::live_tx_detail_screen(
                        chain,
                        hash,
                        rpc.clone(),
                        etherscan_key.clone(),
                        tx_services.clone(),
                    )
                });
                let screen =
                    BlockDetailScreen::loading(chain, BlockId::Number(*number), feed, open_tx);
                Some(match services {
                    Some(s) => Box::new(screen.with_cursor_services(s)),
                    None => Box::new(screen),
                })
            }
            NavigableValue::BlockHash(hash) => {
                let reader = crate::adapters::rpc::AlchemyBlockReader::new(self.rpc.clone());
                let (feed, sender) = block_feed();
                std::mem::drop(super::block_feed::spawn(chain, reader, sender));
                let rpc = self.rpc.clone();
                let etherscan_key = self.etherscan_key.clone();
                let tx_services = services.clone();
                let open_tx = Box::new(move |h| {
                    super::live_tx_detail_screen(
                        chain,
                        h,
                        rpc.clone(),
                        etherscan_key.clone(),
                        tx_services.clone(),
                    )
                });
                let screen = BlockDetailScreen::loading(chain, BlockId::Hash(*hash), feed, open_tx);
                Some(match services {
                    Some(s) => Box::new(screen.with_cursor_services(s)),
                    None => Box::new(screen),
                })
            }
        }
    }
}

/// Helper used by `build_live_stack` to build a `CursorServices`
/// bundle with the live clipboard adapter and navigation factory.
///
/// Logs whether the `ArboardClipboard` acquired a live handle so the
/// user can diagnose a missing DISPLAY / WAYLAND_DISPLAY the next
/// time `y` silently falls back.
#[must_use]
pub fn live_cursor_services(
    rpc: RpcClient,
    alchemy_key: String,
    etherscan_key: Option<String>,
    chain: Chain,
) -> CursorServices {
    let arboard = crate::adapters::clipboard::ArboardClipboard::new();
    if arboard.is_live() {
        tracing::info!(
            target: "blockexplorer_tui::clipboard",
            "arboard clipboard handle acquired; `y` copies will reach the OS clipboard",
        );
    }
    let clipboard: Arc<dyn ClipboardPort> = Arc::new(arboard);
    let factory: Arc<LiveNavigationFactory> =
        Arc::new(LiveNavigationFactory::new(rpc, alchemy_key, etherscan_key));
    let nav: Arc<dyn NavigationFactory> = factory.clone();
    let services = CursorServices::new(clipboard, nav, chain);
    // Hand the factory its own reference to the services so screens
    // opened through `Enter` also carry the live clipboard handle.
    factory.bind_services(services.clone());
    services
}
