//! Composition-root helper that implements `HealthPort` on top of
//! the Alchemy and Etherscan health probes.
//!
//! Either probe may be absent (Etherscan requires an API key; the
//! demo binary never builds it). In that case we surface a stable
//! `HealthLevel::Degraded` entry instead of hiding the row so the
//! Settings screen always has something to render.
//!
//! See `plan/10-settings.md` section 12.5.

use crate::{
    adapters::{etherscan::EtherscanHealth, rpc::AlchemyHealth},
    application::ports::{HealthPort, HealthStatus},
    domain::{Chain, DomainError},
};

/// Wraps the concrete probes so the UI can depend on a single
/// `HealthPort` trait rather than on two adapters.
#[derive(Debug, Clone)]
pub struct CompositeHealth {
    alchemy: Option<AlchemyHealth>,
    etherscan: Option<EtherscanHealth>,
}

impl CompositeHealth {
    #[must_use]
    pub fn new(alchemy: Option<AlchemyHealth>, etherscan: Option<EtherscanHealth>) -> Self {
        Self { alchemy, etherscan }
    }
}

impl HealthPort for CompositeHealth {
    async fn alchemy(&self, chain: Chain) -> Result<HealthStatus, DomainError> {
        match &self.alchemy {
            Some(probe) => probe.probe(chain).await,
            None => Ok(HealthStatus::degraded(
                crate::adapters::rpc::ALCHEMY_PROVIDER,
                0,
                "no Alchemy key configured",
            )),
        }
    }

    async fn etherscan(&self, chain: Chain) -> Result<HealthStatus, DomainError> {
        match &self.etherscan {
            Some(probe) => probe.probe(chain).await,
            None => Ok(HealthStatus::degraded(
                crate::adapters::etherscan::ETHERSCAN_PROVIDER,
                0,
                "no Etherscan key configured",
            )),
        }
    }
}
