//! Alchemy JSON-RPC HTTP adapter.
//!
//! Implements the node-level ports currently required by the Home and
//! Search screens. WebSocket subscriptions are a future concern.
//!
//! See `plan/13-alchemy-adapter.md` and `plan/2-search.md` section 10.2.

pub mod address_lookup;
pub mod block_lookup;
pub mod client;
pub mod ens;
pub mod gas_oracle;
pub mod network_status;
pub mod tx_lookup;

pub use address_lookup::AlchemyAddressLookup;
pub use block_lookup::AlchemyBlockLookup;
pub use client::{RpcClient, RpcError};
pub use ens::AlchemyEnsResolver;
pub use gas_oracle::AlchemyGasOracleAdapter;
pub use network_status::AlchemyNetworkStatusAdapter;
pub use tx_lookup::AlchemyTxLookup;
