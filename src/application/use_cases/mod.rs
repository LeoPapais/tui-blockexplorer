//! Use case implementations.
//!
//! Each file mirrors a use case named in a plan file under `plan/`.

// Opt this module tree into `clippy::pedantic`. See
// `plan/11-rust-scaffolding.md` §9.1.
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::similar_names)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::match_same_arms)]

pub mod classify_address;
pub mod invoke_read_function;
pub mod load_address_overview;
pub mod load_address_portfolio;
pub mod load_address_transfers;
pub mod load_block_overview;
pub mod load_block_transactions;
pub mod load_block_withdrawals;
pub mod load_contract_events_page;
pub mod load_contract_overview;
pub mod load_token_overview;
pub mod load_token_price;
pub mod load_token_price_history;
pub mod load_token_transfers;
pub mod load_tx_overview;
pub mod observe_gas_oracle;
pub mod observe_network_status;
pub mod observe_new_heads;
pub mod resolve_query;
pub mod switch_chain;
