//! Application layer: outbound ports, use cases and screen coordinators.
//!
//! See `.cursor/rules/architecture.mdc` for the full contract.

pub mod home;
pub mod ports;
pub mod use_cases;

pub use home::{ConnectionStatus, HomeSession, HomeViewModel};
