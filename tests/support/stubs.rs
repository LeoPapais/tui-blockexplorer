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
    application::ports::{ChainRegistryPort, GasOraclePort, NetworkStatusPort},
    domain::{BlockNumber, Chain, DomainError, GasSnapshot, Gwei, NetworkStatus, Wei},
};
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
