//! Stub implementations of application ports.
//!
//! Each stub keeps canned responses in a shared `Mutex<...>` so test
//! scenarios can prime the data before calling the real application code.
//! See `.cursor/rules/testing.mdc`.

#![allow(dead_code)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use blockexplorer_tui::{
    application::ports::{
        AddressLookupPort, BlockLookupPort, BlockReaderPort, ChainRegistryPort,
        EnsResolverPort, GasOraclePort, NetworkStatusPort, PendingTxStreamPort,
        TokenSearchPort, TxLookupPort, TxReaderPort,
    },
    domain::{
        Address, AddressKind, Block, BlockHash, BlockId, BlockNumber, BlockSummary, Chain,
        DomainError, GasSnapshot, Gwei, NetworkStatus, PendingTx, PendingTxEvent,
        PendingTxFilter, TokenMetadata, Transaction, TxHash, TxSummary, Wei,
    },
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use serde::Deserialize;

use super::fixture_loader;

// ---------------------------------------------------------------------------
// Fixture DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct NetworkStatusFixture {
    pub chain: String,
    pub latest_block: u64,
    pub base_fee_wei: u128,
    pub block_time_avg_ms: u64,
}

impl NetworkStatusFixture {
    pub fn into_domain(self) -> NetworkStatus {
        NetworkStatus {
            chain: Chain::from_slug(&self.chain).expect("fixture references a known chain"),
            latest_block: BlockNumber::new(self.latest_block),
            base_fee: Wei::new(self.base_fee_wei),
            block_time_avg_ms: self.block_time_avg_ms,
        }
    }

    pub fn load(path: &str) -> NetworkStatus {
        let f: Self = fixture_loader::load_json(path);
        f.into_domain()
    }
}

#[derive(Debug, Deserialize)]
pub struct GasSnapshotFixture {
    pub chain: String,
    pub slow_gwei: u128,
    pub average_gwei: u128,
    pub fast_gwei: u128,
    pub base_fee_gwei: u128,
    pub trend_gwei: Vec<u128>,
}

impl GasSnapshotFixture {
    pub fn into_domain(self) -> GasSnapshot {
        GasSnapshot {
            chain: Chain::from_slug(&self.chain).expect("fixture references a known chain"),
            slow: Gwei::new(self.slow_gwei),
            average: Gwei::new(self.average_gwei),
            fast: Gwei::new(self.fast_gwei),
            base_fee: Gwei::new(self.base_fee_gwei),
            trend: self.trend_gwei.into_iter().map(Gwei::new).collect(),
        }
    }

    pub fn load(path: &str) -> GasSnapshot {
        let f: Self = fixture_loader::load_json(path);
        f.into_domain()
    }
}

// ---------------------------------------------------------------------------
// Stub: NetworkStatusPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct NetworkStatusState {
    by_chain: HashMap<Chain, NetworkStatus>,
    broken: bool,
}

#[derive(Default, Clone)]
pub struct StubNetworkStatusPort {
    inner: Arc<Mutex<NetworkStatusState>>,
}

impl StubNetworkStatusPort {
    pub fn new() -> Self {
        Self::default()
    }

    /// Prime the stub with a canned snapshot for a chain.
    pub fn set_snapshot(&self, status: NetworkStatus) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_chain.insert(status.chain, status);
    }

    /// Put the stub into a broken state. Subsequent calls return
    /// `DomainError::ProviderUnavailable`.
    pub fn set_broken(&self, broken: bool) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.broken = broken;
    }

    /// Read the canned value the test primed for a chain. Panics if no
    /// fixture was set: a missing fixture is always a test bug.
    pub fn expected(&self, chain: Chain) -> NetworkStatus {
        let state = self.inner.lock().expect("stub lock poisoned");
        state
            .by_chain
            .get(&chain)
            .cloned()
            .unwrap_or_else(|| panic!("no network-status fixture primed for {}", chain.slug()))
    }
}

impl NetworkStatusPort for StubNetworkStatusPort {
    async fn snapshot(&self, chain: Chain) -> Result<NetworkStatus, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        if state.broken {
            return Err(DomainError::ProviderUnavailable);
        }
        state.by_chain.get(&chain).cloned().ok_or(DomainError::NotFound)
    }
}

// ---------------------------------------------------------------------------
// Stub: GasOraclePort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct GasOracleState {
    by_chain: HashMap<Chain, GasSnapshot>,
    broken: bool,
}

#[derive(Default, Clone)]
pub struct StubGasOraclePort {
    inner: Arc<Mutex<GasOracleState>>,
}

impl StubGasOraclePort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_snapshot(&self, snapshot: GasSnapshot) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_chain.insert(snapshot.chain, snapshot);
    }

    pub fn set_broken(&self, broken: bool) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.broken = broken;
    }

    pub fn expected(&self, chain: Chain) -> GasSnapshot {
        let state = self.inner.lock().expect("stub lock poisoned");
        state
            .by_chain
            .get(&chain)
            .cloned()
            .unwrap_or_else(|| panic!("no gas-snapshot fixture primed for {}", chain.slug()))
    }
}

impl GasOraclePort for StubGasOraclePort {
    async fn snapshot(&self, chain: Chain) -> Result<GasSnapshot, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        if state.broken {
            return Err(DomainError::ProviderUnavailable);
        }
        state.by_chain.get(&chain).cloned().ok_or(DomainError::NotFound)
    }
}

// ---------------------------------------------------------------------------
// Stub: ChainRegistryPort
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct StubChainRegistry {
    enabled: Vec<Chain>,
    default_chain: Chain,
}

impl StubChainRegistry {
    pub fn new(enabled: Vec<Chain>, default_chain: Chain) -> Self {
        assert!(
            enabled.contains(&default_chain),
            "default chain must be enabled"
        );
        Self {
            enabled,
            default_chain,
        }
    }

    /// Default registry used by most scenarios. Enables every chain in
    /// [`Chain::all`] and picks Ethereum as the default.
    pub fn with_all_enabled() -> Self {
        Self::new(Chain::all().to_vec(), Chain::Ethereum)
    }
}

impl Default for StubChainRegistry {
    fn default() -> Self {
        Self::with_all_enabled()
    }
}

impl ChainRegistryPort for StubChainRegistry {
    fn list_enabled(&self) -> Vec<Chain> {
        self.enabled.clone()
    }

    fn default_chain(&self) -> Chain {
        self.default_chain
    }

    fn ensure_enabled(&self, chain: Chain) -> Result<(), DomainError> {
        if self.enabled.contains(&chain) {
            Ok(())
        } else {
            Err(DomainError::FeatureUnavailable)
        }
    }
}

// ---------------------------------------------------------------------------
// Stub: TxLookupPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TxLookupState {
    by_hash: HashMap<TxHash, TxSummary>,
}

#[derive(Default, Clone)]
pub struct StubTxLookupPort {
    inner: Arc<Mutex<TxLookupState>>,
}

impl StubTxLookupPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, summary: TxSummary) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_hash.insert(summary.hash, summary);
    }
}

impl TxLookupPort for StubTxLookupPort {
    async fn get(
        &self,
        hash: TxHash,
        _chain: Chain,
    ) -> Result<Option<TxSummary>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.by_hash.get(&hash).cloned())
    }
}

// ---------------------------------------------------------------------------
// Stub: BlockLookupPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct BlockLookupState {
    by_hash: HashMap<BlockHash, BlockSummary>,
    by_number: HashMap<BlockNumber, BlockSummary>,
}

#[derive(Default, Clone)]
pub struct StubBlockLookupPort {
    inner: Arc<Mutex<BlockLookupState>>,
}

impl StubBlockLookupPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, summary: BlockSummary) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_hash.insert(summary.hash, summary.clone());
        state.by_number.insert(summary.number, summary);
    }
}

impl BlockLookupPort for StubBlockLookupPort {
    async fn get_by_hash(
        &self,
        hash: BlockHash,
        _chain: Chain,
    ) -> Result<Option<BlockSummary>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.by_hash.get(&hash).cloned())
    }

    async fn get_by_number(
        &self,
        number: BlockNumber,
        _chain: Chain,
    ) -> Result<Option<BlockSummary>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.by_number.get(&number).cloned())
    }
}

// ---------------------------------------------------------------------------
// Stub: AddressLookupPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct AddressLookupState {
    kinds: HashMap<Address, AddressKind>,
}

#[derive(Default, Clone)]
pub struct StubAddressLookupPort {
    inner: Arc<Mutex<AddressLookupState>>,
}

impl StubAddressLookupPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_kind(&self, address: Address, kind: AddressKind) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.kinds.insert(address, kind);
    }
}

impl AddressLookupPort for StubAddressLookupPort {
    async fn classify(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<AddressKind, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        // Default: unknown addresses are EOAs. Tests that care prime
        // the stub explicitly.
        Ok(state
            .kinds
            .get(&address)
            .copied()
            .unwrap_or(AddressKind::Eoa))
    }
}

// ---------------------------------------------------------------------------
// Stub: EnsResolverPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct EnsState {
    forward: HashMap<String, Address>,
    reverse: HashMap<Address, String>,
}

#[derive(Default, Clone)]
pub struct StubEnsResolverPort {
    inner: Arc<Mutex<EnsState>>,
}

impl StubEnsResolverPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_forward(&self, name: &str, address: Address) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.forward.insert(name.to_lowercase(), address);
    }

    pub fn set_reverse(&self, address: Address, name: &str) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.reverse.insert(address, name.to_string());
    }
}

impl EnsResolverPort for StubEnsResolverPort {
    async fn forward(
        &self,
        name: &str,
        _chain: Chain,
    ) -> Result<Option<Address>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.forward.get(&name.to_lowercase()).copied())
    }

    async fn reverse(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<Option<String>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.reverse.get(&address).cloned())
    }
}

// ---------------------------------------------------------------------------
// Stub: TokenSearchPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TokenSearchState {
    by_symbol: HashMap<String, Vec<TokenMetadata>>,
    by_name: HashMap<String, Vec<TokenMetadata>>,
}

#[derive(Default, Clone)]
pub struct StubTokenSearchPort {
    inner: Arc<Mutex<TokenSearchState>>,
}

impl StubTokenSearchPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_symbol(&self, symbol: &str, matches: Vec<TokenMetadata>) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_symbol.insert(symbol.to_uppercase(), matches);
    }

    pub fn set_name(&self, text: &str, matches: Vec<TokenMetadata>) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_name.insert(text.to_lowercase(), matches);
    }
}

impl TokenSearchPort for StubTokenSearchPort {
    async fn by_symbol(
        &self,
        symbol: &str,
        _chain: Chain,
    ) -> Result<Vec<TokenMetadata>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state
            .by_symbol
            .get(&symbol.to_uppercase())
            .cloned()
            .unwrap_or_default())
    }

    async fn by_name(
        &self,
        text: &str,
        _chain: Chain,
    ) -> Result<Vec<TokenMetadata>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state
            .by_name
            .get(&text.to_lowercase())
            .cloned()
            .unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// Stub: BlockReaderPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct BlockReaderState {
    by_number: HashMap<BlockNumber, Block>,
    by_hash: HashMap<BlockHash, Block>,
}

#[derive(Default, Clone)]
pub struct StubBlockReaderPort {
    inner: Arc<Mutex<BlockReaderState>>,
}

impl StubBlockReaderPort {
    pub fn new() -> Self {
        Self::default()
    }

    /// Prime the stub with a full [`Block`] reachable through either
    /// number or hash.
    pub fn insert(&self, block: Block) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_number.insert(block.number, block.clone());
        state.by_hash.insert(block.hash, block);
    }
}

impl BlockReaderPort for StubBlockReaderPort {
    async fn get(&self, id: BlockId, _chain: Chain) -> Result<Option<Block>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        let value = match id {
            BlockId::Number(n) => state.by_number.get(&n).cloned(),
            BlockId::Hash(h) => state.by_hash.get(&h).cloned(),
        };
        Ok(value)
    }
}

// ---------------------------------------------------------------------------
// Stub: TxReaderPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TxReaderState {
    by_hash: HashMap<TxHash, Transaction>,
}

#[derive(Default, Clone)]
pub struct StubTxReaderPort {
    inner: Arc<Mutex<TxReaderState>>,
}

impl StubTxReaderPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, tx: Transaction) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_hash.insert(tx.hash, tx);
    }
}

impl TxReaderPort for StubTxReaderPort {
    async fn get(
        &self,
        hash: TxHash,
        _chain: Chain,
    ) -> Result<Option<Transaction>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.by_hash.get(&hash).cloned())
    }
}

// ---------------------------------------------------------------------------
// Stub: PendingTxStreamPort
// ---------------------------------------------------------------------------

/// Single-subscriber stub that exposes helpers to push `Added` /
/// `Removed` events into the channel the screen drains. Multiple
/// `subscribe` calls each get their own channel; only the latest one
/// retains the sender handle the test drives through.
#[derive(Default, Clone)]
pub struct StubPendingTxStreamPort {
    inner: Arc<Mutex<Vec<UnboundedSender<PendingTxEvent>>>>,
}

impl StubPendingTxStreamPort {
    pub fn new() -> Self {
        Self::default()
    }

    /// Deliver an `Added` event to every live subscriber.
    pub fn push_added(&self, tx: PendingTx) {
        self.broadcast(PendingTxEvent::Added(tx));
    }

    /// Deliver a `Removed` event to every live subscriber.
    pub fn push_removed(&self, hash: TxHash) {
        self.broadcast(PendingTxEvent::Removed(hash));
    }

    fn broadcast(&self, event: PendingTxEvent) {
        let mut senders = self.inner.lock().expect("stub lock poisoned");
        senders.retain(|s| s.send(event.clone()).is_ok());
    }
}

impl PendingTxStreamPort for StubPendingTxStreamPort {
    async fn subscribe(
        &self,
        _chain: Chain,
        _filter: PendingTxFilter,
    ) -> Result<UnboundedReceiver<PendingTxEvent>, DomainError> {
        let (tx, rx) = unbounded_channel();
        let mut senders = self.inner.lock().expect("stub lock poisoned");
        senders.push(tx);
        Ok(rx)
    }
}
