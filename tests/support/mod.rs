//! Shared test helpers: fixture loader, port stubs and the canned
//! WebSocket harness used by Alchemy WebSocket adapter tests.
//!
//! See `.cursor/rules/testing.mdc` for the contract.

pub mod fixture_loader;
pub mod stubs;
#[allow(dead_code)]
// Used by functional tests for WebSocket adapters; the e2e binary pulls
// in `tests/support` wholesale, so clippy may see items as dead when
// building that target.
pub mod ws_harness;
