//! Binary entry point. Delegates to [`blockexplorer_tui::infra`] which owns
//! the composition root and Tokio runtime bootstrap.

fn main() -> anyhow::Result<()> {
    blockexplorer_tui::infra::run()
}
