//! Adapter implementations of application ports.
//!
//! UI is kept isolated from outbound adapters: `adapters::ui` must not
//! import other `adapters::*` modules.

pub mod cache;
pub mod clock;
pub mod config;
pub mod ens;
pub mod etherscan;
pub mod labels;
pub mod prices;
pub mod rng;
pub mod rpc;
pub mod secrets;
pub mod signatures;
pub mod ui;
