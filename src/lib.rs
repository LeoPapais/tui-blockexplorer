//! Root library for `blockexplorer-tui`.
//!
//! The crate is organized following hexagonal architecture:
//! * [`domain`] holds pure value objects, entities and domain errors.
//! * [`application`] defines outbound ports and use cases that orchestrate
//!   them.
//! * [`adapters`] implement ports (RPC, Etherscan, signatures, ENS, cache,
//!   UI, config).
//! * [`infra`] is the composition root.
//!
//! See [`plan/0-general-architecture.md`](../plan/0-general-architecture.md)
//! for the full picture.

pub mod adapters;
pub mod application;
pub mod domain;
pub mod infra;
