//! Shared test helpers: fixture loader, port stubs and the canned
//! WebSocket harness used by the Alchemy pending-tx adapter tests.
//!
//! See `.cursor/rules/testing.mdc` for the contract.

pub mod fixture_loader;
pub mod stubs;
#[allow(dead_code)]
// Used by the functional test binary (see
// tests/functional/alchemy_pending_tx_stream.rs); the e2e binary
// pulls in `tests/support` wholesale, so clippy sees the items as
// dead when building that target. The allow is scoped to the module
// so stubs and fixtures keep their own dead-code checks.
pub mod ws_harness;
