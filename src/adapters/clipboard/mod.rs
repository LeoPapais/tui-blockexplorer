//! Clipboard adapters.
//!
//! Production: [`arboard::ArboardClipboard`]. Tests supply their own
//! stub (`tests/support/stubs.rs::StubClipboard`). See
//! `plan/17-navigable-values.md` §5.

pub mod arboard;

pub use arboard::ArboardClipboard;
