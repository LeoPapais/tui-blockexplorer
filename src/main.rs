//! Binary entry point. Delegates to [`blockexplorer_tui::infra`] which owns
//! the composition root and Tokio runtime bootstrap.

// Mirror the crate-wide lint floor from `src/lib.rs`. `main.rs` is a
// separate compilation unit, so inner attributes on `lib.rs` do not
// reach it. See `plan/11-rust-scaffolding.md` §9.1.
#![deny(clippy::dbg_macro)]
#![warn(clippy::todo)]

fn main() -> anyhow::Result<()> {
    blockexplorer_tui::infra::run()
}
