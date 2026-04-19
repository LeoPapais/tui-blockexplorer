//! Functional tests for the pure `gas::convert_unit` helper.
//!
//! See `plan/9-gas-tracker.md` §11.4 and the `ConvertUnits` use case in §4.2.

use blockexplorer_tui::domain::{DomainError, gas};
use pretty_assertions::assert_eq;
use rstest::rstest;

#[rstest]
#[case("1", gas::Unit::Ether, gas::Unit::Wei, "1000000000000000000")]
#[case("1", gas::Unit::Ether, gas::Unit::Gwei, "1000000000")]
#[case("1", gas::Unit::Gwei, gas::Unit::Wei, "1000000000")]
#[case("1000000000", gas::Unit::Wei, gas::Unit::Gwei, "1")]
#[case("1000000000000000000", gas::Unit::Wei, gas::Unit::Ether, "1")]
#[case("1.5", gas::Unit::Ether, gas::Unit::Gwei, "1500000000")]
#[case("1.5", gas::Unit::Gwei, gas::Unit::Wei, "1500000000")]
#[case("0.000000001", gas::Unit::Ether, gas::Unit::Wei, "1000000000")]
#[case("0.000000001", gas::Unit::Ether, gas::Unit::Gwei, "1")]
#[case("2000000001", gas::Unit::Wei, gas::Unit::Gwei, "2.000000001")]
#[case("0", gas::Unit::Ether, gas::Unit::Wei, "0")]
#[case("42", gas::Unit::Gwei, gas::Unit::Gwei, "42")]
#[case("  1  ", gas::Unit::Ether, gas::Unit::Wei, "1000000000000000000")]
fn it_converts_valid_inputs(
    #[case] value: &str,
    #[case] from: gas::Unit,
    #[case] to: gas::Unit,
    #[case] expected: &str,
) {
    let got = gas::convert_unit(value, from, to).expect("valid input");
    assert_eq!(got, expected);
}

#[test]
fn it_rejects_negative_values() {
    let err = gas::convert_unit("-1", gas::Unit::Ether, gas::Unit::Wei)
        .expect_err("negative must be rejected");
    let msg = match err {
        DomainError::InvalidInput(m) => m,
        other => panic!("expected InvalidInput, got {other:?}"),
    };
    assert!(
        msg.contains("negative"),
        "expected message to mention negative, got {msg:?}",
    );
}

#[rstest]
#[case("")]
#[case("   ")]
#[case("abc")]
#[case("1.2.3")]
#[case(".")]
#[case("+1")]
fn it_rejects_garbage_inputs(#[case] value: &str) {
    let err = gas::convert_unit(value, gas::Unit::Ether, gas::Unit::Wei)
        .expect_err("garbage must be rejected");
    assert!(matches!(err, DomainError::InvalidInput(_)));
}

#[test]
fn it_rejects_input_precision_exceeding_source_decimals() {
    // Wei has 0 fractional decimals.
    let err = gas::convert_unit("1.1", gas::Unit::Wei, gas::Unit::Gwei)
        .expect_err("fractional wei is not a thing");
    assert!(matches!(err, DomainError::InvalidInput(_)));

    // Gwei has 9 fractional decimals.
    let err = gas::convert_unit("1.0000000001", gas::Unit::Gwei, gas::Unit::Wei)
        .expect_err("10 fractional digits for gwei must fail");
    assert!(matches!(err, DomainError::InvalidInput(_)));
}

#[test]
fn it_rejects_more_than_18_fractional_digits_for_ether() {
    let err = gas::convert_unit("1.0000000000000000001", gas::Unit::Ether, gas::Unit::Wei)
        .expect_err("19 fractional digits exceed ether's precision");
    assert!(matches!(err, DomainError::InvalidInput(_)));
}

#[test]
fn it_reports_overflow_on_large_ether_values() {
    // u128::MAX wei is ~3.4e20 ether. 1e21 ether overflows.
    let huge = "1".to_string() + &"0".repeat(21);
    let err = gas::convert_unit(&huge, gas::Unit::Ether, gas::Unit::Wei)
        .expect_err("overflow must be reported");
    let msg = match err {
        DomainError::InvalidInput(m) => m,
        other => panic!("expected InvalidInput, got {other:?}"),
    };
    assert!(
        msg.contains("u128") || msg.contains("overflow"),
        "expected overflow message, got {msg:?}",
    );
}
