//! Use case implementations.
//!
//! Each file mirrors a use case named in a plan file under `plan/`.

pub mod load_address_overview;
pub mod load_block_overview;
pub mod load_contract_overview;
pub mod load_tx_overview;
pub mod observe_gas_oracle;
pub mod observe_network_status;
pub mod observe_pending_txs;
pub mod resolve_query;
pub mod switch_chain;
