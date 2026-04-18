//! Price value objects for the Token Detail screen.
//!
//! See `plan/8-token-detail.md` section 12.4. `PriceWindow` intentionally
//! lands with three variants (not four) because the UI exposes three
//! buttons: 1d / 1m / 1y with matching 1h / 1d / 1w granularity.

use crate::domain::UnixTimestamp;

/// Spot price for a token quoted in one fiat currency.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenPrice {
    /// Lowercase ISO-ish currency code. `"usd"` for MVP.
    pub currency: String,
    /// Numeric value. Floats are used because USD prices can span
    /// many orders of magnitude (wrapped ETH ~2k, shitcoin ~1e-9);
    /// precision beyond ~15 significant digits is not meaningful for
    /// human display anyway.
    pub value: f64,
    /// Timestamp returned by the provider.
    pub as_of: UnixTimestamp,
}

/// Result of a spot-price lookup.
///
/// A token either has a price with the provider (`Available`), is
/// known not to be indexed by the provider (`Unsupported`, carrying
/// the provider label so the UI can tell the user which source
/// declined to answer), or the lookup is still in flight
/// (`Pending`).
///
/// See `plan/15-backlog.md` §3.4.
#[derive(Debug, Clone, PartialEq)]
pub enum PriceLookup {
    Available(TokenPrice),
    Unsupported { provider: &'static str },
    Pending,
}

impl PriceLookup {
    /// Accessor for the spot price when the lookup is `Available`.
    /// Returns `None` for both `Unsupported` and `Pending` to match
    /// the "no data" path at the UI layer.
    #[must_use]
    pub fn as_available(&self) -> Option<&TokenPrice> {
        match self {
            PriceLookup::Available(p) => Some(p),
            _ => None,
        }
    }
}

/// Time range the historical chart can display. The variant order
/// matches the UI binding (`1` → D1, `2` → M1, `3` → Y1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PriceWindow {
    /// Last 24 hours, sampled at 1-hour granularity.
    D1,
    /// Last 30 days, sampled at 1-day granularity.
    M1,
    /// Last 365 days, sampled at 1-week granularity.
    Y1,
}

impl PriceWindow {
    /// Short human-readable label used by the UI (tab titles,
    /// keybinding hints).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            PriceWindow::D1 => "1d",
            PriceWindow::M1 => "1m",
            PriceWindow::Y1 => "1y",
        }
    }

    /// All variants in UI order. Keeps the `1..=3` key mapping in
    /// one place.
    #[must_use]
    pub const fn all() -> &'static [PriceWindow] {
        &[PriceWindow::D1, PriceWindow::M1, PriceWindow::Y1]
    }

    /// Total duration covered by the window, in seconds.
    #[must_use]
    pub const fn span_seconds(self) -> u64 {
        match self {
            PriceWindow::D1 => 24 * 60 * 60,
            PriceWindow::M1 => 30 * 24 * 60 * 60,
            PriceWindow::Y1 => 365 * 24 * 60 * 60,
        }
    }

    /// Granularity string understood by the Alchemy Prices API.
    /// See <https://www.alchemy.com/docs/data> → Prices API.
    #[must_use]
    pub const fn alchemy_interval(self) -> &'static str {
        match self {
            PriceWindow::D1 => "1h",
            PriceWindow::M1 => "1d",
            PriceWindow::Y1 => "1w",
        }
    }
}

/// Single point in a [`PriceSeries`]. `value` is in the parent
/// series' currency.
#[derive(Debug, Clone, PartialEq)]
pub struct PricePoint {
    pub at: UnixTimestamp,
    pub value: f64,
}

/// Ordered list of historical price samples for one token.
/// Adapters return points in chronological (ascending) order.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceSeries {
    pub window: PriceWindow,
    pub currency: String,
    pub points: Vec<PricePoint>,
}

impl PriceSeries {
    /// Upper bound on the number of points the Token Detail chart
    /// keeps as a rolling live tail. See
    /// `plan/8-token-detail.md` §13.1 — the screen appends one point
    /// per streamed `Available` sample and drops the oldest when the
    /// total exceeds this cap.
    pub const ROLLING_CAP: usize = 60;

    /// Convenience: an empty series for a given window, used by the
    /// UI during the loading state.
    #[must_use]
    pub fn empty(window: PriceWindow) -> Self {
        Self {
            window,
            currency: "usd".to_string(),
            points: Vec::new(),
        }
    }

    /// `(min, max)` value across the series, ignoring non-finite
    /// samples. Returns `None` on an empty series.
    #[must_use]
    pub fn y_bounds(&self) -> Option<(f64, f64)> {
        let mut iter = self
            .points
            .iter()
            .map(|p| p.value)
            .filter(|v| v.is_finite());
        let first = iter.next()?;
        let (mut lo, mut hi) = (first, first);
        for v in iter {
            if v < lo {
                lo = v;
            }
            if v > hi {
                hi = v;
            }
        }
        Some((lo, hi))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_seconds_and_interval_cover_every_window() {
        for w in PriceWindow::all() {
            assert!(w.span_seconds() > 0);
            assert!(!w.alchemy_interval().is_empty());
            assert!(!w.label().is_empty());
        }
    }

    #[test]
    fn y_bounds_returns_none_for_empty_series() {
        let s = PriceSeries::empty(PriceWindow::D1);
        assert!(s.y_bounds().is_none());
    }

    #[test]
    fn y_bounds_returns_min_and_max_across_points() {
        let s = PriceSeries {
            window: PriceWindow::D1,
            currency: "usd".into(),
            points: vec![
                PricePoint {
                    at: UnixTimestamp::from_seconds(1),
                    value: 2.0,
                },
                PricePoint {
                    at: UnixTimestamp::from_seconds(2),
                    value: 0.5,
                },
                PricePoint {
                    at: UnixTimestamp::from_seconds(3),
                    value: 1.7,
                },
            ],
        };
        let (lo, hi) = s.y_bounds().expect("non-empty series");
        assert!((lo - 0.5).abs() < 1e-9);
        assert!((hi - 2.0).abs() < 1e-9);
    }
}
