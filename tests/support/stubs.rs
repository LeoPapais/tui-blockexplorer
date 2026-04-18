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
    application::{
        SignatureSource,
        ports::{
            AddressLookupPort, AddressReaderPort, BlockLookupPort, BlockRange, BlockReaderPort,
            ChainRegistryPort, ContractReaderPort, ContractSourcePort, EnsResolverPort,
            EventLogPort, GasOraclePort, NetworkStatusPort, NewHeadsStreamPort,
            PendingTxStreamPort, PortfolioPort, PricesPort, ProxyDetectionPort,
            SignatureDirectoryPort, SignatureHit, StoragePort, TokenReaderPort, TokenSearchPort,
            TransfersPort, TxLookupPort, TxReaderPort, TxSimulationPort, TxTracePort,
        },
    },
    domain::{
        AbiFunction, AbiValue, Address, AddressKind, AddressOverview, AssetChange, Block,
        BlockHash, BlockId, BlockNumber, BlockSummary, Chain, ContractAbi, ContractSource,
        DecodedValue, DomainError, GasSnapshot, Gwei, LogEntry, NetworkStatus, NewHead, PendingTx,
        PendingTxEvent, PendingTxFilter, PriceLookup, PriceSeries, PriceWindow, ProxyInfo,
        StateDiff, TokenHolding, TokenMetadata, TokenOverview, TokenPrice, Transaction,
        TransferCursor, TransferPage, TxHash, TxSummary, Wei,
    },
};
use serde::Deserialize;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

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
        state
            .by_chain
            .get(&chain)
            .cloned()
            .ok_or(DomainError::NotFound)
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
        state
            .by_chain
            .get(&chain)
            .cloned()
            .ok_or(DomainError::NotFound)
    }
}

// ---------------------------------------------------------------------------
// Stub: NewHeadsStreamPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct NewHeadsState {
    senders: Vec<UnboundedSender<NewHead>>,
    broken: bool,
}

/// Fan-out stub for the `newHeads` WebSocket subscription. Each call to
/// [`NewHeadsStreamPort::subscribe`] produces a fresh receiver; calling
/// [`StubNewHeadsStreamPort::push_head`] broadcasts the event to every
/// live subscriber. [`Self::set_broken`] flips subsequent `subscribe`
/// calls to return `DomainError::ProviderUnavailable`, mirroring the
/// behaviour of the real Alchemy WS adapter when it fails to connect.
#[derive(Default, Clone)]
pub struct StubNewHeadsStreamPort {
    inner: Arc<Mutex<NewHeadsState>>,
}

impl StubNewHeadsStreamPort {
    pub fn new() -> Self {
        Self::default()
    }

    /// Deliver a `NewHead` event to every live subscriber. Drops
    /// senders whose receiver has been closed.
    pub fn push_head(&self, head: NewHead) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.senders.retain(|s| s.send(head).is_ok());
    }

    pub fn set_broken(&self, broken: bool) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.broken = broken;
    }

    /// Drop every currently-held sender, simulating the upstream
    /// connection vanishing without an explicit error.
    pub fn disconnect_all(&self) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.senders.clear();
    }
}

impl NewHeadsStreamPort for StubNewHeadsStreamPort {
    async fn subscribe(&self, _chain: Chain) -> Result<UnboundedReceiver<NewHead>, DomainError> {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        if state.broken {
            return Err(DomainError::ProviderUnavailable);
        }
        let (tx, rx) = unbounded_channel();
        state.senders.push(tx);
        Ok(rx)
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
    async fn get(&self, hash: TxHash, _chain: Chain) -> Result<Option<TxSummary>, DomainError> {
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
    async fn classify(&self, address: Address, _chain: Chain) -> Result<AddressKind, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        // Default: unknown addresses are plain EOAs. Tests that care
        // prime the stub explicitly.
        Ok(state
            .kinds
            .get(&address)
            .copied()
            .unwrap_or(AddressKind::Eoa { delegated_to: None }))
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
    async fn forward(&self, name: &str, _chain: Chain) -> Result<Option<Address>, DomainError> {
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

    async fn by_name(&self, text: &str, _chain: Chain) -> Result<Vec<TokenMetadata>, DomainError> {
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
    async fn get(&self, hash: TxHash, _chain: Chain) -> Result<Option<Transaction>, DomainError> {
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

// ---------------------------------------------------------------------------
// Stub: AddressReaderPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct AddressReaderState {
    by_address: HashMap<Address, AddressOverview>,
}

#[derive(Default, Clone)]
pub struct StubAddressReaderPort {
    inner: Arc<Mutex<AddressReaderState>>,
}

impl StubAddressReaderPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, overview: AddressOverview) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_address.insert(overview.address, overview);
    }
}

impl AddressReaderPort for StubAddressReaderPort {
    async fn get(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<Option<AddressOverview>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.by_address.get(&address).cloned())
    }
}

// ---------------------------------------------------------------------------
// Stub: ProxyDetectionPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ProxyDetectionState {
    by_address: HashMap<Address, ProxyInfo>,
}

#[derive(Default, Clone)]
pub struct StubProxyDetectionPort {
    inner: Arc<Mutex<ProxyDetectionState>>,
}

impl StubProxyDetectionPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, address: Address, info: ProxyInfo) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_address.insert(address, info);
    }
}

impl ProxyDetectionPort for StubProxyDetectionPort {
    async fn detect(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<Option<ProxyInfo>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.by_address.get(&address).copied())
    }
}

// ---------------------------------------------------------------------------
// Stub: TokenReaderPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TokenReaderState {
    by_address: HashMap<Address, TokenOverview>,
    /// Number of times `get` has been invoked. Used by tests that
    /// assert on the "pessimistic probing" contract of the search
    /// feed (EOAs must not trigger a probe; contracts do).
    call_count: usize,
}

#[derive(Default, Clone)]
pub struct StubTokenReaderPort {
    inner: Arc<Mutex<TokenReaderState>>,
}

impl StubTokenReaderPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, overview: TokenOverview) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_address.insert(overview.metadata.address, overview);
    }

    /// How many times `get` was called since construction. Exposed
    /// for tests that verify pessimistic probing.
    pub fn call_count(&self) -> usize {
        let state = self.inner.lock().expect("stub lock poisoned");
        state.call_count
    }
}

impl TokenReaderPort for StubTokenReaderPort {
    async fn get(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<Option<TokenOverview>, DomainError> {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.call_count += 1;
        Ok(state.by_address.get(&address).cloned())
    }
}

// ---------------------------------------------------------------------------
// Stub: ContractSourcePort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ContractSourceState {
    abis: HashMap<Address, ContractAbi>,
    sources: HashMap<Address, ContractSource>,
}

#[derive(Default, Clone)]
pub struct StubContractSourcePort {
    inner: Arc<Mutex<ContractSourceState>>,
}

impl StubContractSourcePort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, address: Address, abi: ContractAbi) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.abis.insert(address, abi);
    }

    pub fn insert_source(&self, address: Address, source: ContractSource) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.sources.insert(address, source);
    }
}

impl ContractSourcePort for StubContractSourcePort {
    async fn get_abi(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<Option<ContractAbi>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.abis.get(&address).cloned())
    }

    async fn get_source(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<Option<ContractSource>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.sources.get(&address).cloned())
    }
}

// ---------------------------------------------------------------------------
// Stub: SignatureDirectoryPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct SignatureState {
    selectors: HashMap<[u8; 4], SignatureHit>,
    topics: HashMap<[u8; 32], SignatureHit>,
}

#[derive(Default, Clone)]
pub struct StubSignatureDirectoryPort {
    inner: Arc<Mutex<SignatureState>>,
}

impl StubSignatureDirectoryPort {
    pub fn new() -> Self {
        Self::default()
    }

    /// Prime a selector hit. Defaults the provenance to
    /// [`SignatureSource::Openchain`], matching the live fallback
    /// chain's primary. Use [`Self::set_selector_with_source`] when
    /// a test wants to pin Samczsun as the source.
    pub fn set_selector(&self, selector: [u8; 4], signature: &str) {
        self.set_selector_with_source(selector, signature, SignatureSource::Openchain);
    }

    pub fn set_selector_with_source(
        &self,
        selector: [u8; 4],
        signature: &str,
        source: SignatureSource,
    ) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.selectors.insert(
            selector,
            SignatureHit {
                signature: signature.to_string(),
                source,
            },
        );
    }

    pub fn set_event_topic(&self, topic: [u8; 32], signature: &str) {
        self.set_event_topic_with_source(topic, signature, SignatureSource::Openchain);
    }

    pub fn set_event_topic_with_source(
        &self,
        topic: [u8; 32],
        signature: &str,
        source: SignatureSource,
    ) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.topics.insert(
            topic,
            SignatureHit {
                signature: signature.to_string(),
                source,
            },
        );
    }
}

impl SignatureDirectoryPort for StubSignatureDirectoryPort {
    async fn lookup_selector(
        &self,
        selector: [u8; 4],
    ) -> Result<Option<SignatureHit>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.selectors.get(&selector).cloned())
    }

    async fn lookup_event_topic(
        &self,
        topic: [u8; 32],
    ) -> Result<Option<SignatureHit>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.topics.get(&topic).cloned())
    }
}

// ---------------------------------------------------------------------------
// Stub: TxSimulationPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TxSimulationState {
    by_hash: HashMap<TxHash, Vec<AssetChange>>,
    unsupported: bool,
}

#[derive(Default, Clone)]
pub struct StubTxSimulationPort {
    inner: Arc<Mutex<TxSimulationState>>,
}

impl StubTxSimulationPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_changes(&self, hash: TxHash, changes: Vec<AssetChange>) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_hash.insert(hash, changes);
    }

    pub fn mark_unsupported(&self) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.unsupported = true;
    }
}

impl TxSimulationPort for StubTxSimulationPort {
    async fn simulate_asset_changes(
        &self,
        tx: &Transaction,
        _chain: Chain,
    ) -> Result<Vec<AssetChange>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        if state.unsupported {
            return Err(DomainError::FeatureUnavailable);
        }
        Ok(state.by_hash.get(&tx.hash).cloned().unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// Stub: TxTracePort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TxTraceState {
    by_hash: HashMap<TxHash, StateDiff>,
    unsupported: bool,
}

#[derive(Default, Clone)]
pub struct StubTxTracePort {
    inner: Arc<Mutex<TxTraceState>>,
}

impl StubTxTracePort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_state_diff(&self, hash: TxHash, diff: StateDiff) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_hash.insert(hash, diff);
    }

    pub fn mark_unsupported(&self) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.unsupported = true;
    }
}

impl TxTracePort for StubTxTracePort {
    async fn state_diff(&self, hash: TxHash, _chain: Chain) -> Result<StateDiff, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        if state.unsupported {
            return Err(DomainError::FeatureUnavailable);
        }
        Ok(state.by_hash.get(&hash).cloned().unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// Stub: TransfersPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TransfersState {
    by_address: HashMap<Address, TransferPage>,
    by_contract: HashMap<Address, TransferPage>,
}

#[derive(Default, Clone)]
pub struct StubTransfersPort {
    inner: Arc<Mutex<TransfersState>>,
}

impl StubTransfersPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_page(&self, address: Address, page: TransferPage) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_address.insert(address, page);
    }

    pub fn set_page_for_contract(&self, contract: Address, page: TransferPage) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_contract.insert(contract, page);
    }
}

impl TransfersPort for StubTransfersPort {
    async fn get_for_address(
        &self,
        address: Address,
        _chain: Chain,
        _cursor: Option<TransferCursor>,
    ) -> Result<TransferPage, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.by_address.get(&address).cloned().unwrap_or_default())
    }

    async fn get_for_contract(
        &self,
        contract: Address,
        _chain: Chain,
        _cursor: Option<TransferCursor>,
    ) -> Result<TransferPage, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state
            .by_contract
            .get(&contract)
            .cloned()
            .unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// Stub: PricesPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct PricesState {
    by_address: HashMap<Address, PriceLookup>,
    history: HashMap<(Address, PriceWindow), PriceSeries>,
}

#[derive(Default, Clone)]
pub struct StubPricesPort {
    inner: Arc<Mutex<PricesState>>,
}

impl StubPricesPort {
    pub fn new() -> Self {
        Self::default()
    }

    /// Prime an `Available` price for this contract.
    pub fn set_single(&self, address: Address, price: TokenPrice) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state
            .by_address
            .insert(address, PriceLookup::Available(price));
    }

    /// Prime an `Unsupported` lookup for this contract. Mirrors the
    /// Alchemy Prices 404 path covered by
    /// `plan/15-backlog.md` §3.4.
    pub fn set_unsupported(&self, address: Address, provider: &'static str) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state
            .by_address
            .insert(address, PriceLookup::Unsupported { provider });
    }

    pub fn set_history(&self, address: Address, series: PriceSeries) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.history.insert((address, series.window), series);
    }
}

impl PricesPort for StubPricesPort {
    async fn get_single(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<PriceLookup, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state
            .by_address
            .get(&address)
            .cloned()
            .unwrap_or(PriceLookup::Unsupported {
                provider: "alchemy-prices",
            }))
    }

    async fn get_history(
        &self,
        address: Address,
        _chain: Chain,
        window: PriceWindow,
    ) -> Result<PriceSeries, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state
            .history
            .get(&(address, window))
            .cloned()
            .unwrap_or_else(|| PriceSeries::empty(window)))
    }
}

// ---------------------------------------------------------------------------
// Stub: PortfolioPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct PortfolioState {
    by_address: HashMap<Address, Vec<TokenHolding>>,
}

#[derive(Default, Clone)]
pub struct StubPortfolioPort {
    inner: Arc<Mutex<PortfolioState>>,
}

impl StubPortfolioPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_holdings(&self, address: Address, holdings: Vec<TokenHolding>) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_address.insert(address, holdings);
    }
}

impl PortfolioPort for StubPortfolioPort {
    async fn get_token_balances(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<Vec<TokenHolding>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.by_address.get(&address).cloned().unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// Stub: ContractReaderPort
// ---------------------------------------------------------------------------

type ContractCallKey = (Address, String);

#[derive(Default)]
struct ContractReaderState {
    by_call: HashMap<ContractCallKey, Result<Vec<DecodedValue>, DomainError>>,
}

#[derive(Default, Clone)]
pub struct StubContractReaderPort {
    inner: Arc<Mutex<ContractReaderState>>,
}

impl StubContractReaderPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_result(&self, address: Address, signature: &str, result: Vec<DecodedValue>) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state
            .by_call
            .insert((address, signature.to_string()), Ok(result));
    }

    pub fn set_revert(&self, address: Address, signature: &str, reason: &str) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_call.insert(
            (address, signature.to_string()),
            Err(DomainError::ExecutionReverted {
                reason: reason.to_string(),
            }),
        );
    }
}

impl ContractReaderPort for StubContractReaderPort {
    async fn call(
        &self,
        address: Address,
        _chain: Chain,
        function: &AbiFunction,
        _args: Vec<AbiValue>,
    ) -> Result<Vec<DecodedValue>, DomainError> {
        let key = (address, function.signature());
        let state = self.inner.lock().expect("stub lock poisoned");
        match state.by_call.get(&key) {
            Some(Ok(v)) => Ok(v.clone()),
            Some(Err(DomainError::ExecutionReverted { reason })) => {
                Err(DomainError::ExecutionReverted {
                    reason: reason.clone(),
                })
            }
            Some(Err(_)) | None => Err(DomainError::NotFound),
        }
    }
}

// ---------------------------------------------------------------------------
// Stub: EventLogPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct EventLogState {
    by_address: HashMap<Address, Vec<LogEntry>>,
}

#[derive(Default, Clone)]
pub struct StubEventLogPort {
    inner: Arc<Mutex<EventLogState>>,
}

impl StubEventLogPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_logs(&self, address: Address, logs: Vec<LogEntry>) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_address.insert(address, logs);
    }
}

impl EventLogPort for StubEventLogPort {
    async fn get_logs(
        &self,
        address: Address,
        _chain: Chain,
        _range: BlockRange,
    ) -> Result<Vec<LogEntry>, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.by_address.get(&address).cloned().unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// Stub: StoragePort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct StorageState {
    by_slot: HashMap<(Address, [u8; 32]), [u8; 32]>,
}

#[derive(Default, Clone)]
pub struct StubStoragePort {
    inner: Arc<Mutex<StorageState>>,
}

impl StubStoragePort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_value(&self, address: Address, slot: [u8; 32], value: [u8; 32]) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_slot.insert((address, slot), value);
    }
}

impl StoragePort for StubStoragePort {
    async fn get_at(
        &self,
        address: Address,
        _chain: Chain,
        slot: [u8; 32],
    ) -> Result<[u8; 32], DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state
            .by_slot
            .get(&(address, slot))
            .copied()
            .unwrap_or([0u8; 32]))
    }
}
