//! Gas-related value objects: `Wei` and `Gwei` newtypes, plus pure
//! helpers consumed by the Gas Tracker screen.
//!
//! The domain never leaks a plain `u128` to callers. See
//! `plan/9-gas-tracker.md` §4.2 (`ConvertUnits`), §11.3 (percentiles)
//! and §11.4 (unit converter modal).

use crate::domain::DomainError;

/// Wei is the atomic unit on EVM chains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Wei(u128);

/// Gwei is one billion wei. Most gas numbers in the UI are rendered in gwei.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Gwei(u128);

const WEI_PER_GWEI: u128 = 1_000_000_000;

impl Wei {
    /// Build a `Wei` from a raw `u128` count.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Extract the raw wei count.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }

    /// Convert to gwei, truncating any fractional remainder. This is the
    /// right behaviour for display purposes; a separate conversion should be
    /// added if we ever need rounding or exact decimal arithmetic.
    #[must_use]
    pub const fn to_gwei(self) -> Gwei {
        Gwei(self.0 / WEI_PER_GWEI)
    }
}

impl Gwei {
    /// Build a `Gwei` from a raw `u128` count.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Extract the raw gwei count.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }

    /// Convert to wei exactly.
    #[must_use]
    pub const fn to_wei(self) -> Wei {
        Wei(self.0 * WEI_PER_GWEI)
    }
}

impl From<u128> for Wei {
    fn from(value: u128) -> Self {
        Self(value)
    }
}

impl From<u128> for Gwei {
    fn from(value: u128) -> Self {
        Self(value)
    }
}

// ---------------------------------------------------------------------------
// Unit conversion (`ConvertUnits` use case — plan/9 §4.2, §11.4)
// ---------------------------------------------------------------------------

/// One of the three units the Gas Tracker converter exposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    Wei,
    Gwei,
    Ether,
}

impl Unit {
    /// How many fractional decimal places a string in this unit can
    /// carry before it overflows the next-smaller unit. Equivalent to
    /// `log10(scale_to(Wei))`.
    #[must_use]
    pub const fn decimals(self) -> u32 {
        match self {
            Unit::Wei => 0,
            Unit::Gwei => 9,
            Unit::Ether => 18,
        }
    }

    /// Short human label for the UI.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Unit::Wei => "wei",
            Unit::Gwei => "gwei",
            Unit::Ether => "ether",
        }
    }

    /// Cycle to the next unit in `Wei -> Gwei -> Ether -> Wei` order.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Unit::Wei => Unit::Gwei,
            Unit::Gwei => Unit::Ether,
            Unit::Ether => Unit::Wei,
        }
    }

    /// Cycle in the reverse order.
    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Unit::Wei => Unit::Ether,
            Unit::Gwei => Unit::Wei,
            Unit::Ether => Unit::Gwei,
        }
    }
}

/// Convert `value` (in `from` units) to a canonical decimal string in
/// `to` units.
///
/// Validation rules — documented in `plan/9-gas-tracker.md` §11.4:
/// - inputs with a leading `-` are rejected;
/// - at most one decimal point is allowed;
/// - the fractional part cannot exceed `from.decimals()` digits (so
///   `1.1 wei` is rejected but `0.000000001 ether` is accepted and
///   converts exactly to `1 gwei`);
/// - arithmetic overflow of the internal `u128` wei representation
///   surfaces as `DomainError::InvalidInput` with an "exceeds u128"
///   message.
///
/// # Errors
///
/// Returns [`DomainError::InvalidInput`] when any of the above rules is
/// violated.
pub fn convert_unit(value: &str, from: Unit, to: Unit) -> Result<String, DomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DomainError::InvalidInput("empty input".into()));
    }
    if trimmed.starts_with('-') {
        return Err(DomainError::InvalidInput(
            "negative values not allowed".into(),
        ));
    }
    if trimmed.starts_with('+') {
        return Err(DomainError::InvalidInput(
            "leading '+' is not allowed".into(),
        ));
    }

    // Parse into (integer_part, fractional_part) with at most one dot.
    let (int_part, frac_part) = match trimmed.split_once('.') {
        Some((a, b)) => (a, b),
        None => (trimmed, ""),
    };
    if int_part.contains('.') || frac_part.contains('.') {
        return Err(DomainError::InvalidInput(
            "more than one decimal point".into(),
        ));
    }
    if int_part.is_empty() && frac_part.is_empty() {
        return Err(DomainError::InvalidInput("empty number".into()));
    }
    if !int_part.chars().all(|c| c.is_ascii_digit()) && !int_part.is_empty() {
        return Err(DomainError::InvalidInput(
            "non-digit in integer part".into(),
        ));
    }
    if !frac_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(DomainError::InvalidInput(
            "non-digit in fractional part".into(),
        ));
    }
    if frac_part.len() > from.decimals() as usize {
        return Err(DomainError::InvalidInput(format!(
            "{} supports at most {} fractional digits",
            from.label(),
            from.decimals(),
        )));
    }

    // Normalise to wei. Internal arithmetic is u128.
    let int_u = if int_part.is_empty() {
        0u128
    } else {
        int_part
            .parse::<u128>()
            .map_err(|_| DomainError::InvalidInput("value exceeds u128 range".into()))?
    };
    let frac_u = if frac_part.is_empty() {
        0u128
    } else {
        frac_part
            .parse::<u128>()
            .map_err(|_| DomainError::InvalidInput("value exceeds u128 range".into()))?
    };
    let from_scale = pow10(from.decimals())
        .ok_or_else(|| DomainError::InvalidInput("value exceeds u128 range".into()))?;
    let frac_scale = pow10(frac_part.len() as u32)
        .ok_or_else(|| DomainError::InvalidInput("value exceeds u128 range".into()))?;
    // wei = int_u * from_scale + frac_u * (from_scale / frac_scale)
    let int_contrib = int_u
        .checked_mul(from_scale)
        .ok_or_else(|| DomainError::InvalidInput("value exceeds u128 range".into()))?;
    // from_scale >= frac_scale because frac_part.len() <= from.decimals().
    let frac_multiplier = from_scale / frac_scale;
    let frac_contrib = frac_u
        .checked_mul(frac_multiplier)
        .ok_or_else(|| DomainError::InvalidInput("value exceeds u128 range".into()))?;
    let wei = int_contrib
        .checked_add(frac_contrib)
        .ok_or_else(|| DomainError::InvalidInput("value exceeds u128 range".into()))?;

    // Render in target units.
    let to_scale = pow10(to.decimals())
        .ok_or_else(|| DomainError::InvalidInput("value exceeds u128 range".into()))?;
    let int_out = wei / to_scale;
    let frac_out = wei % to_scale;
    Ok(format_decimal(int_out, frac_out, to.decimals()))
}

fn pow10(exp: u32) -> Option<u128> {
    10u128.checked_pow(exp)
}

fn format_decimal(int_part: u128, frac_part: u128, decimals: u32) -> String {
    if decimals == 0 || frac_part == 0 {
        return int_part.to_string();
    }
    let frac_digits = format!("{frac_part:0>width$}", width = decimals as usize);
    let trimmed = frac_digits.trim_end_matches('0');
    if trimmed.is_empty() {
        int_part.to_string()
    } else {
        format!("{int_part}.{trimmed}")
    }
}

// ---------------------------------------------------------------------------
// Percentile histogram helper (plan/9 §11.3)
// ---------------------------------------------------------------------------

/// Nearest-rank percentiles over a slice of [`Gwei`] samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Percentiles {
    pub p25: Gwei,
    pub p50: Gwei,
    pub p75: Gwei,
}

impl Percentiles {
    /// All-zero percentiles; returned when the input is empty.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            p25: Gwei::new(0),
            p50: Gwei::new(0),
            p75: Gwei::new(0),
        }
    }
}

/// Nearest-rank percentiles over `samples`. Returns
/// [`Percentiles::empty`] when `samples` is empty.
///
/// The nearest-rank method is the definition the RPC layer already
/// relies on for `eth_feeHistory`'s `rewardPercentiles`, so the two
/// histograms line up.
#[must_use]
pub fn percentiles(samples: &[Gwei]) -> Percentiles {
    if samples.is_empty() {
        return Percentiles::empty();
    }
    let mut sorted: Vec<Gwei> = samples.to_vec();
    sorted.sort();
    Percentiles {
        p25: nearest_rank(&sorted, 25),
        p50: nearest_rank(&sorted, 50),
        p75: nearest_rank(&sorted, 75),
    }
}

fn nearest_rank(sorted: &[Gwei], percent: u32) -> Gwei {
    debug_assert!(!sorted.is_empty());
    let n = sorted.len() as u128;
    // rank = ceil(percent / 100 * n); 1-indexed.
    let numerator = (percent as u128) * n;
    let mut rank = numerator / 100;
    if !numerator.is_multiple_of(100) {
        rank += 1;
    }
    if rank == 0 {
        rank = 1;
    }
    let idx = (rank - 1) as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gwei_wei_roundtrip_holds_for_exact_values() {
        let g = Gwei::new(14);
        assert_eq!(g.to_wei().to_gwei(), g);
    }

    #[test]
    fn wei_to_gwei_truncates() {
        let w = Wei::new(WEI_PER_GWEI + 999);
        assert_eq!(w.to_gwei(), Gwei::new(1));
    }

    #[test]
    fn unit_cycle_is_a_three_cycle() {
        assert_eq!(Unit::Wei.next(), Unit::Gwei);
        assert_eq!(Unit::Gwei.next(), Unit::Ether);
        assert_eq!(Unit::Ether.next(), Unit::Wei);
        assert_eq!(Unit::Wei.prev(), Unit::Ether);
    }

    #[test]
    fn format_decimal_trims_trailing_zeros() {
        assert_eq!(format_decimal(1, 500_000_000, 9), "1.5");
        assert_eq!(format_decimal(2, 0, 9), "2");
        assert_eq!(format_decimal(0, 1, 9), "0.000000001");
    }
}
