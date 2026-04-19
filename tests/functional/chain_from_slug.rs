//! `Chain::from_slug` error reporting.
//!
//! `BLOCKEXPLORER_TUI_CHAIN` is documented as accepting one of the
//! supported slugs (`ethereum | ethereum-sepolia | base | polygon |
//! optimism | arbitrum`). When the env var holds an unknown value we
//! want the error message to enumerate the valid options so the user
//! can correct the variable without grepping the codebase.
//!
//! See `plan/14-config-and-credentials.md` §8.2.

use blockexplorer_tui::domain::{Chain, DomainError};
use pretty_assertions::assert_eq;

#[test]
fn unknown_slug_lists_every_supported_chain() {
    let err = Chain::from_slug("solana").expect_err("unknown slug must error");

    let DomainError::InvalidInput(message) = err else {
        panic!("expected InvalidInput, got {err:?}");
    };

    // Every known slug must appear in the message so the user sees
    // the full list without consulting the docs.
    for chain in Chain::all() {
        assert!(
            message.contains(chain.slug()),
            "message {message:?} must mention {}",
            chain.slug()
        );
    }

    // The offending input itself is echoed back.
    assert!(
        message.contains("solana"),
        "message {message:?} must quote the invalid slug"
    );
}

#[test]
fn empty_slug_also_reports_valid_options() {
    let err = Chain::from_slug("").expect_err("empty slug must error");
    let DomainError::InvalidInput(message) = err else {
        panic!("expected InvalidInput, got {err:?}");
    };

    assert!(
        message.contains("ethereum"),
        "message {message:?} must mention at least `ethereum`"
    );
}

#[test]
fn every_known_slug_round_trips() {
    // Regression guard: keeping the error helpful only works if the
    // happy path stays happy for every documented chain.
    for chain in Chain::all() {
        let parsed = Chain::from_slug(chain.slug()).expect("known slug parses");
        assert_eq!(parsed, *chain);
    }
}
