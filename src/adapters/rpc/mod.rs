//! Alchemy JSON-RPC HTTP adapter.
//!
//! Implements the node-level ports currently required by the Home
//! screen. WebSocket subscriptions are a future concern.
//!
//! See `plan/13-alchemy-adapter.md`.

pub mod client;
pub mod gas_oracle;
pub mod network_status;

pub use client::{RpcClient, RpcError};
pub use gas_oracle::AlchemyGasOracleAdapter;
pub use network_status::AlchemyNetworkStatusAdapter;
