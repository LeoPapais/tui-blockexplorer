//! Domain-level error type.
//!
//! Every adapter maps its concrete errors into one of the variants below
//! before returning across a port boundary. Use cases therefore only ever
//! handle [`DomainError`].

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("not found")]
    NotFound,

    #[error("feature unavailable on the active chain")]
    FeatureUnavailable,

    #[error("execution reverted: {reason}")]
    ExecutionReverted { reason: String },

    #[error("provider unavailable")]
    ProviderUnavailable,

    #[error("config error: {0}")]
    Config(String),

    #[error("internal error: {0}")]
    Internal(String),
}
