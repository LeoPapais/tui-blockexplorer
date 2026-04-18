//! Functional test binary. Each file under `tests/functional/` is wired in
//! here as a submodule. See `.cursor/rules/testing.mdc`.

#[path = "support/mod.rs"]
mod support;

#[path = "functional/observe_network_status.rs"]
mod observe_network_status;

#[path = "functional/observe_gas_oracle.rs"]
mod observe_gas_oracle;

#[path = "functional/switch_chain.rs"]
mod switch_chain;

#[path = "functional/home_session.rs"]
mod home_session;

#[path = "functional/home_screen_render.rs"]
mod home_screen_render;

#[path = "functional/screen_stack.rs"]
mod screen_stack;

#[path = "functional/home_screen_keys.rs"]
mod home_screen_keys;

#[path = "functional/alchemy_network_status.rs"]
mod alchemy_network_status;

#[path = "functional/alchemy_gas_oracle.rs"]
mod alchemy_gas_oracle;

#[path = "functional/config_load.rs"]
mod config_load;

#[path = "functional/home_feed.rs"]
mod home_feed;

#[path = "functional/resolve_query.rs"]
mod resolve_query;

#[path = "functional/alchemy_lookups.rs"]
mod alchemy_lookups;

#[path = "functional/load_block_overview.rs"]
mod load_block_overview;

#[path = "functional/alchemy_block_reader.rs"]
mod alchemy_block_reader;

#[path = "functional/load_tx_overview.rs"]
mod load_tx_overview;

#[path = "functional/alchemy_tx_reader.rs"]
mod alchemy_tx_reader;

#[path = "functional/observe_pending_txs.rs"]
mod observe_pending_txs;

#[path = "functional/load_address_overview.rs"]
mod load_address_overview;

#[path = "functional/alchemy_address_reader.rs"]
mod alchemy_address_reader;

#[path = "functional/load_contract_overview.rs"]
mod load_contract_overview;

#[path = "functional/alchemy_proxy_detector.rs"]
mod alchemy_proxy_detector;

#[path = "functional/load_token_overview.rs"]
mod load_token_overview;

#[path = "functional/alchemy_token_reader.rs"]
mod alchemy_token_reader;

#[path = "functional/etherscan_contract_source.rs"]
mod etherscan_contract_source;

#[path = "functional/sourcify_signatures.rs"]
mod sourcify_signatures;

#[path = "functional/alchemy_tx_simulation.rs"]
mod alchemy_tx_simulation;

#[path = "functional/alchemy_tx_trace.rs"]
mod alchemy_tx_trace;

#[path = "functional/load_address_transfers.rs"]
mod load_address_transfers;

#[path = "functional/alchemy_transfers.rs"]
mod alchemy_transfers;

#[path = "functional/load_address_portfolio.rs"]
mod load_address_portfolio;

#[path = "functional/alchemy_portfolio.rs"]
mod alchemy_portfolio;

#[path = "functional/parse_abi_functions.rs"]
mod parse_abi_functions;

#[path = "functional/invoke_read_function.rs"]
mod invoke_read_function;

#[path = "functional/alchemy_contract_reader.rs"]
mod alchemy_contract_reader;

#[path = "functional/alchemy_event_log.rs"]
mod alchemy_event_log;

#[path = "functional/alchemy_storage.rs"]
mod alchemy_storage;

#[path = "functional/load_token_price.rs"]
mod load_token_price;

#[path = "functional/load_token_price_history.rs"]
mod load_token_price_history;

#[path = "functional/load_token_transfers.rs"]
mod load_token_transfers;

#[path = "functional/alchemy_prices.rs"]
mod alchemy_prices;

#[path = "functional/alchemy_transfers_for_contract.rs"]
mod alchemy_transfers_for_contract;

#[path = "functional/search_feed_token_probe.rs"]
mod search_feed_token_probe;

#[path = "functional/tx_detail_screen_keys.rs"]
mod tx_detail_screen_keys;
