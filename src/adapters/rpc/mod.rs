//! Alchemy JSON-RPC HTTP adapter.
//!
//! Implements the node-level ports currently required by the Home and
//! Search screens. WebSocket subscriptions are a future concern.
//!
//! See `plan/13-alchemy-adapter.md` and `plan/2-search.md` section 10.2.

pub mod address_lookup;
pub mod address_reader;
pub mod block_lookup;
pub mod block_reader;
pub mod block_receipts;
pub mod client;
pub mod contract_reader;
pub mod ens;
pub mod event_log;
pub mod gas_oracle;
pub mod network_status;
pub mod portfolio;
pub mod proxy_detection;
pub mod storage;
pub mod token_reader;
pub mod transfers;
pub mod tx_lookup;
pub mod tx_reader;
pub mod tx_simulation;
pub mod tx_trace;

pub use address_lookup::AlchemyAddressLookup;
pub use address_reader::AlchemyAddressReader;
pub use block_lookup::AlchemyBlockLookup;
pub use block_reader::AlchemyBlockReader;
pub use block_receipts::AlchemyBlockReceipts;
pub use client::{RpcClient, RpcError};
pub use contract_reader::AlchemyContractReader;
pub use ens::AlchemyEnsResolver;
pub use event_log::AlchemyEventLog;
pub use gas_oracle::AlchemyGasOracleAdapter;
pub use network_status::AlchemyNetworkStatusAdapter;
pub use portfolio::AlchemyPortfolio;
pub use proxy_detection::AlchemyProxyDetector;
pub use storage::AlchemyStorage;
pub use token_reader::AlchemyTokenReader;
pub use transfers::AlchemyTransfers;
pub use tx_lookup::AlchemyTxLookup;
pub use tx_reader::AlchemyTxReader;
pub use tx_simulation::AlchemySimulation;
pub use tx_trace::AlchemyTxTracer;
