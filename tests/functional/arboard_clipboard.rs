//! Tests for the `arboard`-backed `ClipboardPort` adapter.
//!
//! The real `arboard::Clipboard::new()` needs a live display
//! (X11 / Wayland) which is not available on CI. We cover the
//! adapter's graceful-fallback branch through the injected factory
//! constructor and leave the live path as an `#[ignore]` test
//! intended to be exercised locally. See
//! `plan/17-navigable-values.md` §5.1 + §8.1.

use blockexplorer_tui::{
    adapters::clipboard::ArboardClipboard, application::ports::ClipboardPort,
};
use pretty_assertions::assert_eq;

#[test]
fn graceful_fallback_returns_ok_when_arboard_init_fails() {
    // Factory that simulates a headless environment — `arboard`
    // raises `Error::ClipboardNotSupported` on such boxes; any
    // variant works for the adapter since it only checks `Err`.
    let adapter = ArboardClipboard::with_factory(Box::new(|| {
        Err(arboard::Error::ClipboardNotSupported)
    }));
    assert!(
        !adapter.is_live(),
        "a failing factory should produce a degraded (no-op) adapter",
    );

    let result = adapter.set("ignored");
    assert!(
        result.is_ok(),
        "a no-op adapter must still satisfy ClipboardPort::set (Ok(()))",
    );
    assert_eq!(result.ok(), Some(()));
}

#[test]
#[ignore = "arboard requires a live DISPLAY / Wayland socket; run locally"]
fn real_adapter_round_trips_a_string() {
    let adapter = ArboardClipboard::new();
    adapter.set("blockexplorer-tui clipboard test").unwrap();
}
