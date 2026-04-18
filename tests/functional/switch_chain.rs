//! Functional tests for the `switch_chain` use case.
//!
//! See `plan/1-home.md` section 4.3.

use blockexplorer_tui::{
    application::use_cases::switch_chain,
    domain::{Chain, DomainError},
};

use crate::support::stubs::StubChainRegistry;

#[test]
fn accepts_a_chain_present_in_the_registry() {
    let registry = StubChainRegistry::new(vec![Chain::Ethereum, Chain::Base], Chain::Ethereum);

    let got = switch_chain::run(&registry, Chain::Base);

    assert!(got.is_ok());
}

#[test]
fn rejects_a_chain_missing_from_the_registry() {
    let registry = StubChainRegistry::new(vec![Chain::Ethereum], Chain::Ethereum);

    let err =
        switch_chain::run(&registry, Chain::Base).expect_err("disabled chain must be rejected");

    assert!(matches!(err, DomainError::FeatureUnavailable));
}
