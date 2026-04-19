//! Process-level Alchemy compute-unit meter.
//!
//! Used by the composition root to track per-session CU consumption
//! without plumbing a port through every adapter. The meter is
//! bumped from `RpcClient::call` whenever a call succeeds, using
//! the per-method cost hints from
//! [`crate::adapters::rpc::cost_hint::cost_hint_for`].
//!
//! MVP scope: no UI surface, no per-request attribution. See
//! `plan/13-alchemy-adapter.md` §8.4.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::adapters::rpc::CostRecorder;

/// Monotonic CU counter. Cloneable through `Arc` so many call sites
/// can share the same meter without extra synchronisation.
#[derive(Debug, Default)]
pub struct CostMeter {
    consumed: AtomicU64,
}

impl CostRecorder for CostMeter {
    fn record(&self, compute_units: u32) {
        CostMeter::record(self, compute_units);
    }
}

impl CostMeter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            consumed: AtomicU64::new(0),
        }
    }

    /// Arc-wrapped convenience so the composition root can hand out
    /// a single meter without binding the caller to `Arc<CostMeter>`.
    #[must_use]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// Add `cost` compute units to the running total.
    pub fn record(&self, cost: u32) {
        self.consumed.fetch_add(u64::from(cost), Ordering::Relaxed);
    }

    /// Compute units consumed since construction.
    #[must_use]
    pub fn consumed(&self) -> u64 {
        self.consumed.load(Ordering::Relaxed)
    }

    /// Reset the counter back to zero. Not used by the MVP live
    /// binary; included for tests and future Settings integration.
    pub fn reset(&self) {
        self.consumed.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_zero_and_accumulates_in_record_order() {
        let meter = CostMeter::new();
        assert_eq!(meter.consumed(), 0);
        meter.record(10);
        meter.record(26);
        meter.record(150);
        assert_eq!(meter.consumed(), 186);
    }

    #[test]
    fn reset_zeroes_the_running_total() {
        let meter = CostMeter::new();
        meter.record(42);
        meter.reset();
        assert_eq!(meter.consumed(), 0);
    }

    #[test]
    fn shared_meter_is_observable_across_clones() {
        let meter = CostMeter::shared();
        let clone = meter.clone();
        clone.record(5);
        assert_eq!(meter.consumed(), 5);
    }
}
