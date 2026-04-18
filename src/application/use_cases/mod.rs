//! Use case implementations.
//!
//! Each file mirrors a use case named in a plan file under `plan/`.

pub mod classify_address;
pub mod invoke_read_function;
pub mod load_address_overview;
pub mod load_address_portfolio;
pub mod load_address_transfers;
pub mod load_block_overview;
pub mod load_block_transactions;
pub mod load_contract_overview;
pub mod load_token_overview;
pub mod load_token_price;
pub mod load_token_price_history;
pub mod load_token_transfers;
pub mod load_tx_overview;
pub mod observe_gas_oracle;
pub mod observe_network_status;
pub mod observe_new_heads;
pub mod observe_pending_txs;
pub mod resolve_query;
pub mod switch_chain;
