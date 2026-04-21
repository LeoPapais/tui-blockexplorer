//! Stub implementations of application ports.
//!
//! Each stub keeps canned responses in a shared `Mutex<...>` so test
//! scenarios can prime the data before calling the real application code.
//! See `.cursor/rules/testing.mdc`.

#![allow(dead_code)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use blockexplorer_tui::{
    adapters::ui::{NavigationFactory, Screen},
    application::{
        SignatureSource,
        ports::{
            AccountTransactionsPort, AddressLookupPort, AddressReaderPort, BlockLookupPort,
            BlockRange, BlockReaderPort, BlockReceiptsPort, ChainRegistryPort, ClipboardPort,
            Clock, ContractReaderPort, ContractSourcePort, EnsResolverPort, EventLogPort,
            GasOraclePort, LabelPort, NetworkStatusPort, NewHeadsStreamPort, PortfolioPort,
            PricesPort, ProxyDetectionPort, Rng, SignatureDirectoryPort, SignatureHit, StoragePort,
            TokenPriceStreamPort, TokenReaderPort, TokenSearchPort, TransfersPort, TxLookupPort,
            TxReaderPort, TxSimulationPort, TxTracePort,
        },
    },
    domain::{
        AbiFunction, AbiValue, AccountTxCursor, AccountTxPage, Address, AddressKind,
        AddressOverview, AssetChange, Block, BlockHash, BlockId, BlockNumber, BlockSummary,
        BlockTxReceipt, CallNode, Chain, ContractAbi, ContractSource, DecodedValue, DomainError,
        GasSnapshot, Gwei, Label, LogEntry, NavigableValue, NetworkStatus, NewHead, PriceLookup,
        PriceSeries, PriceWindow, ProxyInfo, StateDiff, TokenHolding, TokenMetadata, TokenOverview,
        TokenPrice, Transaction, TransferCursor, TransferPage, TxHash, TxSummary, Wei,
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
    call_count: usize,
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

    /// Number of times `get` was invoked. Used by the search-cache
    /// tests to assert that a cache hit skips the downstream lookup.
    pub fn call_count(&self) -> usize {
        let state = self.inner.lock().expect("stub lock poisoned");
        state.call_count
    }
}

impl TxLookupPort for StubTxLookupPort {
    async fn get(&self, hash: TxHash, _chain: Chain) -> Result<Option<TxSummary>, DomainError> {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.call_count += 1;
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

    /// Drop every forward mapping. Useful for cache tests that need
    /// to prove the decorator returned a cached answer without
    /// hitting the backing port.
    pub fn clear_forward(&self) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.forward.clear();
    }

    /// Mirror of [`Self::clear_forward`] for the reverse table.
    pub fn clear_reverse(&self) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.reverse.clear();
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
// Stub: BlockReceiptsPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct BlockReceiptsState {
    by_number: HashMap<BlockNumber, Vec<BlockTxReceipt>>,
    by_hash: HashMap<BlockHash, Vec<BlockTxReceipt>>,
    forced_error: Option<DomainError>,
    /// Count of `get_transactions` invocations, used by cancellation
    /// / pagination tests to assert the port was only hit once.
    call_count: usize,
}

#[derive(Default, Clone)]
pub struct StubBlockReceiptsPort {
    inner: Arc<Mutex<BlockReceiptsState>>,
}

impl StubBlockReceiptsPort {
    pub fn new() -> Self {
        Self::default()
    }

    /// Prime the stub with a vector of rows addressable by number.
    pub fn set_by_number(&self, number: BlockNumber, rows: Vec<BlockTxReceipt>) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_number.insert(number, rows);
    }

    /// Prime the stub with a vector of rows addressable by hash.
    pub fn set_by_hash(&self, hash: BlockHash, rows: Vec<BlockTxReceipt>) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_hash.insert(hash, rows);
    }

    /// Prime the stub to bubble up `err` on the next call.
    pub fn fail_with(&self, err: DomainError) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.forced_error = Some(err);
    }

    pub fn call_count(&self) -> usize {
        let state = self.inner.lock().expect("stub lock poisoned");
        state.call_count
    }
}

// ---------------------------------------------------------------------------
// Stub: LabelPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct LabelState {
    by_address: HashMap<Address, Label>,
    /// When set, the next `label_for` call returns the stored
    /// [`DomainError`] instead of consulting the table.
    forced_error: Option<DomainError>,
    call_count: usize,
}

#[derive(Default, Clone)]
pub struct StubLabelPort {
    inner: Arc<Mutex<LabelState>>,
}

impl StubLabelPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_label(&self, address: Address, label: Label) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_address.insert(address, label);
    }

    pub fn fail_with(&self, err: DomainError) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.forced_error = Some(err);
    }

    pub fn call_count(&self) -> usize {
        let state = self.inner.lock().expect("stub lock poisoned");
        state.call_count
    }
}

impl LabelPort for StubLabelPort {
    async fn label_for(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<Option<Label>, DomainError> {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.call_count += 1;
        if let Some(err) = state.forced_error.take() {
            return Err(err);
        }
        Ok(state.by_address.get(&address).cloned())
    }
}

impl BlockReceiptsPort for StubBlockReceiptsPort {
    async fn get_transactions(
        &self,
        id: BlockId,
        _chain: Chain,
    ) -> Result<Vec<BlockTxReceipt>, DomainError> {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.call_count += 1;
        if let Some(err) = state.forced_error.take() {
            return Err(err);
        }
        let rows = match id {
            BlockId::Number(n) => state.by_number.get(&n).cloned(),
            BlockId::Hash(h) => state.by_hash.get(&h).cloned(),
        };
        Ok(rows.unwrap_or_default())
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
    /// Optional delay before answering (plan/18 Slice E: prove bare
    /// `TxView` emission wins the race against decoding).
    delay: Option<Duration>,
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

    /// Hold every lookup for `delay` before returning canned data.
    pub fn set_delay(&self, delay: Duration) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.delay = Some(delay);
    }
}

impl SignatureDirectoryPort for StubSignatureDirectoryPort {
    async fn lookup_selector(
        &self,
        selector: [u8; 4],
    ) -> Result<Option<SignatureHit>, DomainError> {
        let delay = {
            let state = self.inner.lock().expect("stub lock poisoned");
            state.delay
        };
        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.selectors.get(&selector).cloned())
    }

    async fn lookup_event_topic(
        &self,
        topic: [u8; 32],
    ) -> Result<Option<SignatureHit>, DomainError> {
        let delay = {
            let state = self.inner.lock().expect("stub lock poisoned");
            state.delay
        };
        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }
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
    /// Artificial delay injected before answering. Used by
    /// `plan/4-tx-detail.md` section 12.6.1 to prove the Overview
    /// base view reaches the channel before the heavier enrichment
    /// calls return.
    delay: Option<Duration>,
    /// Count of `simulate_asset_changes` invocations since
    /// construction.
    call_count: usize,
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

    /// Hold every incoming call for `delay` before answering. Used
    /// to prove ordering claims (see `plan/4-tx-detail.md` 12.6.1).
    pub fn set_delay(&self, delay: Duration) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.delay = Some(delay);
    }

    pub fn call_count(&self) -> usize {
        let state = self.inner.lock().expect("stub lock poisoned");
        state.call_count
    }
}

impl TxSimulationPort for StubTxSimulationPort {
    async fn simulate_asset_changes(
        &self,
        tx: &Transaction,
        _chain: Chain,
    ) -> Result<Vec<AssetChange>, DomainError> {
        let (delay, unsupported, canned) = {
            let state = self.inner.lock().expect("stub lock poisoned");
            (
                state.delay,
                state.unsupported,
                state.by_hash.get(&tx.hash).cloned().unwrap_or_default(),
            )
        };
        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }
        // Count only calls that finished (ran past the delay). See
        // the sibling comment on `StubTxTracePort::state_diff`.
        self.inner.lock().expect("stub lock poisoned").call_count += 1;
        if unsupported {
            return Err(DomainError::FeatureUnavailable);
        }
        Ok(canned)
    }
}

// ---------------------------------------------------------------------------
// Stub: TxTracePort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TxTraceState {
    by_hash: HashMap<TxHash, StateDiff>,
    call_trees: HashMap<TxHash, CallNode>,
    unsupported: bool,
    /// Artificial delay injected before answering. See the sibling
    /// comment on `StubTxSimulationPort::set_delay`.
    delay: Option<Duration>,
    /// Count of `state_diff` invocations since construction.
    call_count: usize,
    /// Count of `call_tree` invocations since construction. Kept
    /// separate from `call_count` so the Internal-tab tests can
    /// assert on tree access specifically.
    call_tree_count: usize,
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

    /// Prime a call tree for the Internal tab tests (plan 12.6.5).
    pub fn set_call_tree(&self, hash: TxHash, tree: CallNode) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.call_trees.insert(hash, tree);
    }

    pub fn mark_unsupported(&self) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.unsupported = true;
    }

    pub fn set_delay(&self, delay: Duration) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.delay = Some(delay);
    }

    pub fn call_count(&self) -> usize {
        let state = self.inner.lock().expect("stub lock poisoned");
        state.call_count
    }

    pub fn call_tree_count(&self) -> usize {
        let state = self.inner.lock().expect("stub lock poisoned");
        state.call_tree_count
    }
}

impl TxTracePort for StubTxTracePort {
    async fn state_diff(&self, hash: TxHash, _chain: Chain) -> Result<StateDiff, DomainError> {
        let (delay, unsupported, canned) = {
            let state = self.inner.lock().expect("stub lock poisoned");
            (
                state.delay,
                state.unsupported,
                state.by_hash.get(&hash).cloned().unwrap_or_default(),
            )
        };
        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }
        // Count only calls that finished (ran past the delay). This
        // keeps the "call_count == 0" assertion in the ordering
        // tests aligned with "tracer has not finished yet".
        self.inner.lock().expect("stub lock poisoned").call_count += 1;
        if unsupported {
            return Err(DomainError::FeatureUnavailable);
        }
        Ok(canned)
    }

    async fn call_tree(&self, hash: TxHash, _chain: Chain) -> Result<CallNode, DomainError> {
        let (delay, unsupported, canned) = {
            let state = self.inner.lock().expect("stub lock poisoned");
            (
                state.delay,
                state.unsupported,
                state.call_trees.get(&hash).cloned(),
            )
        };
        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }
        self.inner
            .lock()
            .expect("stub lock poisoned")
            .call_tree_count += 1;
        if unsupported {
            return Err(DomainError::FeatureUnavailable);
        }
        canned.ok_or(DomainError::FeatureUnavailable)
    }
}

// ---------------------------------------------------------------------------
// Stub: AccountTransactionsPort
// ---------------------------------------------------------------------------

#[derive(Default)]
struct AccountTransactionsState {
    by_address: HashMap<Address, AccountTxPage>,
}

#[derive(Default, Clone)]
pub struct StubAccountTransactionsPort {
    inner: Arc<Mutex<AccountTransactionsState>>,
}

impl StubAccountTransactionsPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_page(&self, address: Address, page: AccountTxPage) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.by_address.insert(address, page);
    }
}

impl AccountTransactionsPort for StubAccountTransactionsPort {
    async fn list_for_address(
        &self,
        address: Address,
        _chain: Chain,
        _cursor: Option<AccountTxCursor>,
    ) -> Result<AccountTxPage, DomainError> {
        let state = self.inner.lock().expect("stub lock poisoned");
        Ok(state.by_address.get(&address).cloned().unwrap_or_default())
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
// Stub: TokenPriceStreamPort
// ---------------------------------------------------------------------------

/// Fan-out stub for the live token-price subscription. Scenarios push
/// values on demand via [`Self::push`] / [`Self::push_unsupported`];
/// every live receiver gets a copy. [`Self::set_broken`] flips
/// subsequent `subscribe` calls to `DomainError::ProviderUnavailable`,
/// mirroring the behaviour of the real polling adapter when its
/// underlying port is down.
///
/// See `plan/8-token-detail.md` §13.1.
#[derive(Default)]
struct TokenPriceStreamState {
    senders: HashMap<Address, Vec<UnboundedSender<PriceLookup>>>,
    broken: bool,
}

#[derive(Default, Clone)]
pub struct StubTokenPriceStreamPort {
    inner: Arc<Mutex<TokenPriceStreamState>>,
}

impl StubTokenPriceStreamPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_broken(&self, broken: bool) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        state.broken = broken;
    }

    /// Broadcast an `Available` lookup to every live subscriber for
    /// this address.
    pub fn push(&self, address: Address, price: TokenPrice) {
        self.broadcast(address, PriceLookup::Available(price));
    }

    /// Broadcast an `Unsupported` lookup for this address.
    pub fn push_unsupported(&self, address: Address, provider: &'static str) {
        self.broadcast(address, PriceLookup::Unsupported { provider });
    }

    fn broadcast(&self, address: Address, lookup: PriceLookup) {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        if let Some(list) = state.senders.get_mut(&address) {
            list.retain(|s| s.send(lookup.clone()).is_ok());
        }
    }

    /// Count of live subscribers for `address`. BDD steps poll this
    /// to wait for the async dispatcher to have actually subscribed
    /// before pushing a sample, avoiding races between `push` and
    /// `subscribe` landing on the Tokio executor.
    pub fn subscriber_count(&self, address: Address) -> usize {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        if let Some(list) = state.senders.get_mut(&address) {
            list.retain(|s| !s.is_closed());
            list.len()
        } else {
            0
        }
    }
}

impl TokenPriceStreamPort for StubTokenPriceStreamPort {
    async fn subscribe(
        &self,
        address: Address,
        _chain: Chain,
    ) -> Result<UnboundedReceiver<PriceLookup>, DomainError> {
        let mut state = self.inner.lock().expect("stub lock poisoned");
        if state.broken {
            return Err(DomainError::ProviderUnavailable);
        }
        let (tx, rx) = unbounded_channel();
        state.senders.entry(address).or_default().push(tx);
        Ok(rx)
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

// ---------------------------------------------------------------------------
// Stub: Clock
// ---------------------------------------------------------------------------

/// Manually-driven clock used in tests. Wraps a `Mutex<Instant>` so the
/// test can step forward via `advance` without needing to wait for
/// wall-clock time to pass.
///
/// See `plan/2-search.md` section 12.5.
#[derive(Clone)]
pub struct FrozenClock {
    inner: Arc<Mutex<Instant>>,
}

impl FrozenClock {
    /// Construct a clock anchored to `now` (usually `Instant::now()`
    /// captured once at the top of the test).
    #[must_use]
    pub fn at(now: Instant) -> Self {
        Self {
            inner: Arc::new(Mutex::new(now)),
        }
    }

    /// Move the clock forward by `delta`.
    pub fn advance(&self, delta: Duration) {
        let mut guard = self.inner.lock().expect("clock lock poisoned");
        *guard += delta;
    }
}

impl Default for FrozenClock {
    fn default() -> Self {
        Self::at(Instant::now())
    }
}

impl Clock for FrozenClock {
    fn now(&self) -> Instant {
        *self.inner.lock().expect("clock lock poisoned")
    }
}

// ---------------------------------------------------------------------------
// Stub: Rng
// ---------------------------------------------------------------------------

/// Deterministic `Rng` used in tests.
///
/// Wraps a tiny xorshift64* generator so the port has zero external
/// dependencies and the stream is reproducible across test runs.
/// Default seed is `0xdead_beef_cafe_f00d`; use `SeededRng::new(seed)`
/// for per-test seeds.
///
/// See `plan/11-rust-scaffolding.md` §9.3.
#[derive(Clone)]
pub struct SeededRng {
    state: Arc<Mutex<u64>>,
}

impl SeededRng {
    /// Create a stub seeded with the given value.
    ///
    /// # Panics
    ///
    /// Panics if `seed == 0`, which would collapse xorshift64* to the
    /// all-zero fixed point. Use any non-zero seed.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        assert!(seed != 0, "SeededRng: seed must be non-zero");
        Self {
            state: Arc::new(Mutex::new(seed)),
        }
    }

    fn step(state: &mut u64) -> u64 {
        // xorshift64* — small, fast, deterministic. Constants from
        // Marsaglia "Xorshift RNGs" (2003) + Vigna's multiplier.
        let mut x = *state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        *state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

impl Default for SeededRng {
    fn default() -> Self {
        Self::new(0xdead_beef_cafe_f00d)
    }
}

impl Rng for SeededRng {
    fn fill_bytes(&self, dest: &mut [u8]) {
        let mut guard = self.state.lock().expect("rng lock poisoned");
        for chunk in dest.chunks_mut(8) {
            let value = Self::step(&mut guard).to_le_bytes();
            chunk.copy_from_slice(&value[..chunk.len()]);
        }
    }

    fn next_u64(&self) -> u64 {
        let mut guard = self.state.lock().expect("rng lock poisoned");
        Self::step(&mut guard)
    }
}

// ---------------------------------------------------------------------------
// Stub: ClipboardPort
// ---------------------------------------------------------------------------

/// In-memory `ClipboardPort` used by the per-screen cursor tests.
/// Holds the last string `set` received; `last_copied()` returns a
/// clone of it so assertions stay immutable. See
/// `plan/17-navigable-values.md` §5.2.
#[derive(Default, Clone)]
pub struct StubClipboard {
    last: Arc<Mutex<Option<String>>>,
}

impl StubClipboard {
    /// Empty clipboard.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Latest value passed to [`ClipboardPort::set`], or `None` when
    /// the stub was never called.
    #[must_use]
    pub fn last_copied(&self) -> Option<String> {
        self.last
            .lock()
            .expect("stub clipboard lock poisoned")
            .clone()
    }
}

impl ClipboardPort for StubClipboard {
    fn set(&self, text: &str) -> Result<(), DomainError> {
        *self.last.lock().expect("stub clipboard lock poisoned") = Some(text.to_string());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Stub: NavigationFactory
// ---------------------------------------------------------------------------

/// Recording `NavigationFactory` used by per-screen cursor tests.
///
/// Every call to `open` appends the `(value, chain)` pair to an
/// internal log before returning `None` (so the screen stack stays
/// untouched — tests assert on the recording only). See
/// `plan/17-navigable-values.md` §5.3.
#[derive(Default, Clone)]
pub struct StubNavigationFactory {
    recorded: Arc<Mutex<Vec<(NavigableValue, Chain)>>>,
}

impl StubNavigationFactory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot of every `(value, chain)` the screen asked to open.
    #[must_use]
    pub fn recorded(&self) -> Vec<(NavigableValue, Chain)> {
        self.recorded
            .lock()
            .expect("stub navigation lock poisoned")
            .clone()
    }

    /// Convenience for assertions that only care about the values.
    #[must_use]
    pub fn recorded_values(&self) -> Vec<NavigableValue> {
        self.recorded().into_iter().map(|(v, _)| v).collect()
    }
}

impl NavigationFactory for StubNavigationFactory {
    fn open(&self, value: &NavigableValue, active_chain: Chain) -> Option<Box<dyn Screen>> {
        self.recorded
            .lock()
            .expect("stub navigation lock poisoned")
            .push((value.clone(), active_chain));
        None
    }
}
