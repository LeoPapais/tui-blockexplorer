//! Atomic use case: validate that a target chain is enabled in the chain
//! registry. The caller is responsible for tearing down subscriptions and
//! restarting observers; see [`crate::application::home::HomeSession`] for
//! the composed flow.
//!
//! See `plan/1-home.md` section 4.3.

use crate::{
    application::ports::ChainRegistryPort,
    domain::{Chain, DomainError},
};

pub fn run<P: ChainRegistryPort>(registry: &P, target: Chain) -> Result<(), DomainError> {
    registry.ensure_enabled(target)
}
