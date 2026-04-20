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
/// response to `Enter` on a navigable value.
pub struct LiveNavigationFactory {
    rpc: RpcClient,
    alchemy_key: String,
    etherscan_key: Option<String>,
}

impl LiveNavigationFactory {
    #[must_use]
    pub fn new(rpc: RpcClient, alchemy_key: String, etherscan_key: Option<String>) -> Self {
        Self {
            rpc,
            alchemy_key,
            etherscan_key,
        }
    }

    /// Build an `Arc<dyn NavigationFactory>` so a single factory can
    /// be cloned into every screen's [`CursorServices`] bundle.
    #[must_use]
    pub fn arc(self) -> Arc<dyn NavigationFactory> {
        Arc::new(self)
    }
}

impl NavigationFactory for LiveNavigationFactory {
    fn open(&self, value: &NavigableValue, chain: Chain) -> Option<Box<dyn Screen>> {
        match value {
            NavigableValue::Plain(_) => None,
            NavigableValue::Address(addr) => Some(super::live_address_detail_screen(
                chain,
                *addr,
                AddressTab::Overview,
                self.rpc.clone(),
                self.alchemy_key.clone(),
                self.etherscan_key.clone(),
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
            )),
            NavigableValue::TxHash(hash) => Some(super::live_tx_detail_screen(
                chain,
                *hash,
                self.rpc.clone(),
                self.etherscan_key.clone(),
            )),
            NavigableValue::BlockNumber(number) => {
                let reader = crate::adapters::rpc::AlchemyBlockReader::new(self.rpc.clone());
                let (feed, sender) = block_feed();
                std::mem::drop(super::block_feed::spawn(chain, reader, sender));
                let rpc = self.rpc.clone();
                let etherscan_key = self.etherscan_key.clone();
                let open_tx = Box::new(move |hash| {
                    super::live_tx_detail_screen(chain, hash, rpc.clone(), etherscan_key.clone())
                });
                Some(Box::new(BlockDetailScreen::loading(
                    chain,
                    BlockId::Number(*number),
                    feed,
                    open_tx,
                )))
            }
            NavigableValue::BlockHash(hash) => {
                let reader = crate::adapters::rpc::AlchemyBlockReader::new(self.rpc.clone());
                let (feed, sender) = block_feed();
                std::mem::drop(super::block_feed::spawn(chain, reader, sender));
                let rpc = self.rpc.clone();
                let etherscan_key = self.etherscan_key.clone();
                let open_tx = Box::new(move |hash| {
                    super::live_tx_detail_screen(chain, hash, rpc.clone(), etherscan_key.clone())
                });
                Some(Box::new(BlockDetailScreen::loading(
                    chain,
                    BlockId::Hash(*hash),
                    feed,
                    open_tx,
                )))
            }
        }
    }
}

/// Helper used by `build_live_stack` to build a `CursorServices`
/// bundle with the live clipboard adapter and navigation factory.
#[must_use]
pub fn live_cursor_services(
    rpc: RpcClient,
    alchemy_key: String,
    etherscan_key: Option<String>,
    chain: Chain,
) -> CursorServices {
    let clipboard: Arc<dyn ClipboardPort> =
        Arc::new(crate::adapters::clipboard::ArboardClipboard::new());
    let nav = LiveNavigationFactory::new(rpc, alchemy_key, etherscan_key).arc();
    CursorServices::new(clipboard, nav, chain)
}
