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
