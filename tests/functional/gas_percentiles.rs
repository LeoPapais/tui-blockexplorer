//! Functional tests for the pure `gas::percentiles` helper.
//!
//! See `plan/9-gas-tracker.md` §11.3.

use blockexplorer_tui::domain::{Gwei, gas};
use pretty_assertions::assert_eq;

fn g(values: &[u128]) -> Vec<Gwei> {
    values.iter().copied().map(Gwei::new).collect()
}

#[test]
fn empty_input_returns_zeroed_percentiles() {
    let p = gas::percentiles(&[]);
    assert_eq!(p, gas::Percentiles::empty());
    assert_eq!(p.p25, Gwei::new(0));
    assert_eq!(p.p50, Gwei::new(0));
    assert_eq!(p.p75, Gwei::new(0));
}

#[test]
fn single_sample_replicates_across_all_tiers() {
    let p = gas::percentiles(&g(&[42]));
    assert_eq!(p.p25, Gwei::new(42));
    assert_eq!(p.p50, Gwei::new(42));
    assert_eq!(p.p75, Gwei::new(42));
}

#[test]
fn nearest_rank_on_four_samples() {
    // Sorted ascending: 1, 2, 3, 4.
    // Nearest-rank: p25 -> index 0, p50 -> index 1, p75 -> index 2.
    let p = gas::percentiles(&g(&[4, 2, 1, 3]));
    assert_eq!(p.p25, Gwei::new(1));
    assert_eq!(p.p50, Gwei::new(2));
    assert_eq!(p.p75, Gwei::new(3));
}

#[test]
fn nearest_rank_on_five_samples() {
    let p = gas::percentiles(&g(&[5, 4, 3, 2, 1]));
    // ranks: ceil(1.25)=2, ceil(2.5)=3, ceil(3.75)=4 -> values 2/3/4.
    assert_eq!(p.p25, Gwei::new(2));
    assert_eq!(p.p50, Gwei::new(3));
    assert_eq!(p.p75, Gwei::new(4));
}

#[test]
fn original_slice_is_not_mutated() {
    let samples = g(&[9, 1, 5]);
    let _ = gas::percentiles(&samples);
    assert_eq!(samples, g(&[9, 1, 5]));
}
