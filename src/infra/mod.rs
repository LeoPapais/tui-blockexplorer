//! Composition root.
//!
//! Loads configuration, constructs adapters, wires them into use cases,
//! boots the Tokio runtime and starts the TUI. All intentionally
//! placeholder at this scaffolding stage: no runtime behaviour has been
//! implemented yet.

/// Entry point called from `main`. Returns `Ok(())` immediately until
/// the first screen use case is wired up in a future plan phase.
///
/// See `plan/11-rust-scaffolding.md` and `plan/0-general-architecture.md`
/// section 8 for the intended runtime model.
pub fn run() -> anyhow::Result<()> {
    Ok(())
}
