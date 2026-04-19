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

#[path = "functional/mempool_screen.rs"]
mod mempool_screen;

#[path = "functional/mempool_screen_render.rs"]
mod mempool_screen_render;

#[path = "functional/alchemy_pending_tx_stream.rs"]
mod alchemy_pending_tx_stream;

#[path = "functional/observe_new_heads.rs"]
mod observe_new_heads;

#[path = "functional/load_address_overview.rs"]
mod load_address_overview;

#[path = "functional/alchemy_address_reader.rs"]
mod alchemy_address_reader;

#[path = "functional/load_contract_overview.rs"]
mod load_contract_overview;

#[path = "functional/load_contract_events_page.rs"]
mod load_contract_events_page;

#[path = "functional/highlight_solidity.rs"]
mod highlight_solidity;

#[path = "functional/contract_detail_source_highlight.rs"]
mod contract_detail_source_highlight;

#[path = "functional/alchemy_proxy_detector.rs"]
mod alchemy_proxy_detector;

#[path = "functional/composite_proxy_detector.rs"]
mod composite_proxy_detector;

#[path = "functional/etherscan_proxy_hint.rs"]
mod etherscan_proxy_hint;

#[path = "functional/load_token_overview.rs"]
mod load_token_overview;

#[path = "functional/alchemy_token_reader.rs"]
mod alchemy_token_reader;

#[path = "functional/etherscan_contract_source.rs"]
mod etherscan_contract_source;

#[path = "functional/openchain_signatures.rs"]
mod openchain_signatures;

#[path = "functional/composite_signature_directory.rs"]
mod composite_signature_directory;

#[path = "functional/contract_source_proxy_following.rs"]
mod contract_source_proxy_following;

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

#[path = "functional/domain_block.rs"]
mod domain_block;

#[path = "functional/classify_address.rs"]
mod classify_address;

#[path = "functional/ttl_cache.rs"]
mod ttl_cache;

#[path = "functional/etherscan_token_search.rs"]
mod etherscan_token_search;

#[path = "functional/search_feed_cache.rs"]
mod search_feed_cache;

#[path = "functional/block_detail_screen_keys.rs"]
mod block_detail_screen_keys;

#[path = "functional/address_detail_screen_keys.rs"]
mod address_detail_screen_keys;

#[path = "functional/alchemy_ens_reverse.rs"]
mod alchemy_ens_reverse;

#[path = "functional/ens_reverse_cache.rs"]
mod ens_reverse_cache;

#[path = "functional/portfolio_summary.rs"]
mod portfolio_summary;

#[path = "functional/load_block_transactions.rs"]
mod load_block_transactions;

#[path = "functional/alchemy_block_receipts.rs"]
mod alchemy_block_receipts;

#[path = "functional/label_composite.rs"]
mod label_composite;

#[path = "functional/etherscan_label.rs"]
mod etherscan_label;

#[path = "functional/load_block_withdrawals.rs"]
mod load_block_withdrawals;

#[path = "functional/token_price_stream.rs"]
mod token_price_stream;

#[path = "functional/token_detail_screen_keys.rs"]
mod token_detail_screen_keys;

#[path = "functional/gas_unit_conversion.rs"]
mod gas_unit_conversion;

#[path = "functional/gas_percentiles.rs"]
mod gas_percentiles;

#[path = "functional/gas_tracker_screen.rs"]
mod gas_tracker_screen;

#[path = "functional/logger_masking.rs"]
mod logger_masking;

#[path = "functional/home_screen_first_run_banner.rs"]
mod home_screen_first_run_banner;

#[path = "functional/fs_config_save.rs"]
mod fs_config_save;

#[path = "functional/keymap_conflict.rs"]
mod keymap_conflict;

#[path = "functional/alchemy_health.rs"]
mod alchemy_health;

#[path = "functional/etherscan_health.rs"]
mod etherscan_health;
