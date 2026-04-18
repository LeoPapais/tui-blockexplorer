//! Home screen coordinator.
//!
//! Composes [`observe_network_status`](crate::application::use_cases::observe_network_status),
//! [`observe_gas_oracle`](crate::application::use_cases::observe_gas_oracle) and
//! [`switch_chain`](crate::application::use_cases::switch_chain) into a
//! single screen-facing value. Consumed by the UI adapter and by the BDD
//! step definitions.
//!
//! See `plan/1-home.md` section 11.3.

use crate::{
    application::{
        ports::{ChainRegistryPort, GasOraclePort, NetworkStatusPort},
        use_cases::{observe_gas_oracle, observe_network_status, switch_chain},
    },
    domain::{Chain, DomainError, GasSnapshot, NetworkStatus, NewHead},
};

/// High-level connection state surfaced on the Home header.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    #[default]
    Connected,
    Disconnected {
        reconnect_scheduled: bool,
    },
}

/// Everything the Home screen needs to render a frame. Produced by
/// [`HomeSession`] and consumed by the UI adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeViewModel {
    pub chain: Chain,
    pub network: Option<NetworkStatus>,
    pub gas: Option<GasSnapshot>,
    pub connection: ConnectionStatus,
}

impl HomeViewModel {
    fn new(chain: Chain) -> Self {
        Self {
            chain,
            network: None,
            gas: None,
            connection: ConnectionStatus::Connected,
        }
    }
}

/// Screen-scoped coordinator. Owns a handle to each port and keeps the
/// latest [`HomeViewModel`] in memory.
pub struct HomeSession<N, G, C>
where
    N: NetworkStatusPort,
    G: GasOraclePort,
    C: ChainRegistryPort,
{
    network: N,
    gas: G,
    chains: C,
    state: HomeViewModel,
}

impl<N, G, C> HomeSession<N, G, C>
where
    N: NetworkStatusPort,
    G: GasOraclePort,
    C: ChainRegistryPort,
{
    /// Create a session for the given chain. The caller is expected to
    /// invoke [`HomeSession::refresh`] before reading the view model.
    pub fn new(network: N, gas: G, chains: C, chain: Chain) -> Self {
        Self {
            network,
            gas,
            chains,
            state: HomeViewModel::new(chain),
        }
    }

    /// Read-only access to the current view model. The UI adapter uses
    /// this to render a frame.
    #[must_use]
    pub fn view(&self) -> &HomeViewModel {
        &self.state
    }

    /// Pull a fresh snapshot from both observers for the current chain.
    ///
    /// A provider failure leaves the previous values untouched but marks
    /// the connection as disconnected with a reconnect scheduled.
    pub async fn refresh(&mut self) -> Result<(), DomainError> {
        let chain = self.state.chain;
        let network = observe_network_status::run(&self.network, chain).await;
        let gas = observe_gas_oracle::run(&self.gas, chain).await;

        match (network, gas) {
            (Ok(n), Ok(g)) => {
                self.state.network = Some(n);
                self.state.gas = Some(g);
                self.state.connection = ConnectionStatus::Connected;
                Ok(())
            }
            (Err(DomainError::ProviderUnavailable), _)
            | (_, Err(DomainError::ProviderUnavailable)) => {
                self.state.connection = ConnectionStatus::Disconnected {
                    reconnect_scheduled: true,
                };
                Ok(())
            }
            (Err(e), _) | (_, Err(e)) => Err(e),
        }
    }

    /// Hook to be called every time the `newHeads` subscription fires.
    /// Currently equivalent to [`HomeSession::refresh`]; kept separate
    /// because future work may coalesce head-triggered refreshes.
    pub async fn on_new_head(&mut self) -> Result<(), DomainError> {
        self.refresh().await
    }

    /// Called when a live `newHeads` event is delivered by the
    /// WebSocket subscription. Ignores events from other chains (can
    /// happen when a chain switch races a still-draining WS stream)
    /// and otherwise drives a full [`HomeSession::refresh`] to pick up
    /// the new base-fee and block-time average.
    ///
    /// See `plan/1-home.md` section 12.3.
    pub async fn on_new_head_event(&mut self, head: NewHead) -> Result<(), DomainError> {
        if head.chain != self.state.chain {
            return Ok(());
        }
        self.refresh().await
    }

    /// Mark the connection as dropped. Does not attempt to reconnect; the
    /// outer runtime owns reconnection.
    pub fn on_connection_drop(&mut self) {
        self.state.connection = ConnectionStatus::Disconnected {
            reconnect_scheduled: true,
        };
    }

    /// Validate the target chain, switch the session state to it and
    /// refresh from the observers.
    pub async fn switch_chain(&mut self, target: Chain) -> Result<(), DomainError> {
        switch_chain::run(&self.chains, target)?;
        self.state.chain = target;
        self.refresh().await
    }
}
