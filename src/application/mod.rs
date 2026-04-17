//! Application layer: outbound ports and use cases.
//!
//! Use cases depend on port traits, never on concrete adapters.
//! See `.cursor/rules/architecture.mdc` for the full contract.

pub mod ports;
pub mod use_cases;
