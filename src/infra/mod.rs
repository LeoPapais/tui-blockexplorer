//! Composition root.
//!
//! Parses the CLI, builds the initial screen stack and runs the event
//! loop in a Tokio multi-thread runtime.
//!
//! See `plan/12-screen-runtime.md`.

mod runtime;

use anyhow::Result;

use crate::adapters::ui::{HomeScreen, ScreenStack};

/// Hint printed when the binary is invoked without the `--demo` flag
/// (and before the Alchemy adapter lands in phase 2).
const NO_DATA_HINT: &str = "blockexplorer-tui: real data adapters are not wired yet.\n\
Launch with `cargo run -- --demo` to see the Home screen rendered with\n\
placeholder data.\n\
See plan/12-screen-runtime.md (runtime) and plan/13-alchemy-adapter.md\n\
(real data, upcoming).";

/// Entry point called from `main`.
pub fn run() -> Result<()> {
    if !wants_demo() {
        eprintln!("{NO_DATA_HINT}");
        return Ok(());
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run_tui())
}

fn wants_demo() -> bool {
    std::env::args().any(|arg| arg == "--demo")
}

async fn run_tui() -> Result<()> {
    let mut stack = ScreenStack::new();
    stack.push(Box::new(HomeScreen::with_demo_data()));
    runtime::run_event_loop(stack).await
}
