//! Alchemy JSON-RPC HTTP + WebSocket adapter.
//!
//! Implements the node-level ports currently required by the Home and
//! Search screens, plus the `newHeads` WebSocket stream for Home.
//!
//! See `plan/13-alchemy-adapter.md` and `plan/2-search.md` section 10.2.

pub mod address_lookup;
pub mod address_reader;
pub mod block_lookup;
pub mod block_reader;
pub mod block_receipts;
pub mod circuit_breaker;
pub mod client;
pub mod composite_proxy_detection;
pub mod contract_reader;
pub mod cost_hint;
pub mod ens;
pub mod event_log;
pub mod gas_oracle;
pub mod health;
pub mod network_status;
pub mod new_heads_stream;
pub mod portfolio;
pub mod proxy_detection;
pub mod retry;
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
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
pub use client::{CostRecorder, RpcClient, RpcError};
pub use composite_proxy_detection::CompositeProxyDetector;
pub use contract_reader::AlchemyContractReader;
pub use cost_hint::{CostHint, CostKind, cost_hint_for};
pub use ens::AlchemyEnsResolver;
pub use event_log::AlchemyEventLog;
pub use gas_oracle::AlchemyGasOracleAdapter;
pub use health::{ALCHEMY_PROVIDER, AlchemyHealth};
pub use network_status::AlchemyNetworkStatusAdapter;
pub use new_heads_stream::AlchemyNewHeadsStream;
pub use portfolio::AlchemyPortfolio;
pub use proxy_detection::AlchemyProxyDetector;
pub use retry::{RetryPolicy, retry_with_backoff};
pub use storage::AlchemyStorage;
pub use token_reader::AlchemyTokenReader;
pub use transfers::AlchemyTransfers;
pub use tx_lookup::AlchemyTxLookup;
pub use tx_reader::AlchemyTxReader;
pub use tx_simulation::AlchemySimulation;
pub use tx_trace::AlchemyTxTracer;
