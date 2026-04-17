//! Atomic use case: pull the current [`GasSnapshot`] from the
//! [`GasOraclePort`].
//!
//! See `plan/1-home.md` section 4.2.

use crate::{
    application::ports::GasOraclePort,
    domain::{Chain, DomainError, GasSnapshot},
};

pub async fn run<P: GasOraclePort>(port: &P, chain: Chain) -> Result<GasSnapshot, DomainError> {
    port.snapshot(chain).await
}
