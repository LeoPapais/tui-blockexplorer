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

// Crate-wide lint floor. See `plan/11-rust-scaffolding.md` §9.1.
// `dbg!` must never ship; `todo!` is a signal that the author owes a plan
// reference, so we nudge reviewers by warning on every occurrence.
#![deny(clippy::dbg_macro)]
#![warn(clippy::todo)]

pub mod adapters;
pub mod application;
pub mod domain;
pub mod infra;
