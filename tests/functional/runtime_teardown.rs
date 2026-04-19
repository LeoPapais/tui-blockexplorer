//! Functional tests for the runtime teardown helpers.
//!
//! These tests cover the small units that belong to the runtime
//! module: the idempotent terminal teardown flag (panic hook +
//! regular exit path must share it) and the `Ctrl+C` signal → command
//! mapping. They do not spin up a real `Terminal`.
//!
//! See `plan/12-screen-runtime.md` §7 items 1 and 2.

use blockexplorer_tui::infra::runtime::{TeardownGate, signal_to_command};
use blockexplorer_tui::adapters::ui::Command;

#[test]
fn teardown_gate_only_fires_once() {
    let gate = TeardownGate::new();
    assert!(gate.claim());
    assert!(!gate.claim());
    assert!(!gate.claim());
}

#[test]
fn teardown_gate_reports_already_fired_after_claim() {
    let gate = TeardownGate::new();
    assert!(!gate.fired());
    gate.claim();
    assert!(gate.fired());
}

#[test]
fn signal_to_command_maps_to_quit() {
    assert_eq!(signal_to_command(), Command::Quit);
}
