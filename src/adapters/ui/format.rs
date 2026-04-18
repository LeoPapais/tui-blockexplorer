//! Human-readable formatters for Wei / gwei / gas units.
//!
//! See `plan/4-tx-detail.md` section 13.2.
//!
//! Every formatter is pure (no allocation-heavy crates, no clock,
//! no locale) so they can be unit-tested exhaustively and reused
//! across every screen.

use crate::domain::Wei;

/// 10^18, the wei-per-ether scalar.
const WEI_PER_ETH: u128 = 1_000_000_000_000_000_000;

/// 10^9, the wei-per-gwei scalar.
const WEI_PER_GWEI: u128 = 1_000_000_000;

/// Humanize a `Wei` amount into an `ETH` string.
///
/// - integer part is thousands-grouped (US-style commas),
/// - fractional part keeps up to 6 significant decimals, trimmed
///   of trailing zeros,
/// - zero renders as `0 ETH`,
/// - amounts below `0.000001` ETH surface as `<0.000001 ETH` to
///   avoid the visually empty `0 ETH` for tiny but non-zero fees.
#[must_use]
pub fn humanize_eth(wei: Wei) -> String {
    let value = wei.value();
    if value == 0 {
        return "0 ETH".to_string();
    }
    let whole = value / WEI_PER_ETH;
    let frac = value % WEI_PER_ETH;
    let frac_str = format!("{:018}", frac);
    let truncated: String = frac_str.chars().take(6).collect();
    let trimmed = truncated.trim_end_matches('0');
    let integer = thousands(whole);
    if trimmed.is_empty() {
        if whole == 0 {
            "<0.000001 ETH".to_string()
        } else {
            format!("{integer} ETH")
        }
    } else {
        format!("{integer}.{trimmed} ETH")
    }
}

/// Humanize a `Wei` gas-price amount into a `gwei` string.
///
/// - keeps up to 4 decimals, trimmed of trailing zeros,
/// - zero renders as `0 gwei`,
/// - amounts below `0.0001` gwei surface as `<0.0001 gwei`.
#[must_use]
pub fn humanize_gwei(wei: Wei) -> String {
    let value = wei.value();
    if value == 0 {
        return "0 gwei".to_string();
    }
    let whole = value / WEI_PER_GWEI;
    let frac = value % WEI_PER_GWEI;
    let frac_str = format!("{:09}", frac);
    let truncated: String = frac_str.chars().take(4).collect();
    let trimmed = truncated.trim_end_matches('0');
    let integer = thousands(whole);
    if trimmed.is_empty() {
        if whole == 0 {
            "<0.0001 gwei".to_string()
        } else {
            format!("{integer} gwei")
        }
    } else {
        format!("{integer}.{trimmed} gwei")
    }
}

/// Humanize a gas-unit count with thousands grouping.
#[must_use]
pub fn humanize_gas_units(gas: u64) -> String {
    thousands(u128::from(gas))
}

/// US-style thousands grouping for a `u128`.
fn thousands(mut n: u128) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut buf = Vec::new();
    while n > 0 {
        let chunk = (n % 1000) as u16;
        n /= 1000;
        if n == 0 {
            buf.push(format!("{chunk}"));
        } else {
            buf.push(format!("{chunk:03}"));
        }
    }
    buf.reverse();
    buf.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(v: u128) -> Wei {
        Wei::new(v)
    }

    #[test]
    fn humanize_eth_renders_zero() {
        assert_eq!(humanize_eth(w(0)), "0 ETH");
    }

    #[test]
    fn humanize_eth_renders_one_wei_as_sub_threshold() {
        assert_eq!(humanize_eth(w(1)), "<0.000001 ETH");
    }

    #[test]
    fn humanize_eth_renders_sub_eth_amount() {
        // 0.000732 ETH == 732_000_000_000_000 wei.
        assert_eq!(humanize_eth(w(732_000_000_000_000)), "0.000732 ETH");
    }

    #[test]
    fn humanize_eth_renders_exactly_one_eth() {
        assert_eq!(humanize_eth(w(WEI_PER_ETH)), "1 ETH");
    }

    #[test]
    fn humanize_eth_groups_thousands_on_large_values() {
        assert_eq!(humanize_eth(w(1_234_000 * WEI_PER_ETH)), "1,234,000 ETH");
    }

    #[test]
    fn humanize_eth_truncates_decimals_to_six() {
        // 1.2345678 ETH.
        let amount = WEI_PER_ETH + 234_567_800_000_000_000;
        assert_eq!(humanize_eth(w(amount)), "1.234567 ETH");
    }

    #[test]
    fn humanize_eth_trims_trailing_zeros() {
        // 1.5 ETH.
        let amount = WEI_PER_ETH + 500_000_000_000_000_000;
        assert_eq!(humanize_eth(w(amount)), "1.5 ETH");
    }

    #[test]
    fn humanize_eth_handles_exact_fraction_boundary() {
        // 0.1 ETH.
        assert_eq!(humanize_eth(w(WEI_PER_ETH / 10)), "0.1 ETH");
    }

    #[test]
    fn humanize_gwei_renders_zero() {
        assert_eq!(humanize_gwei(w(0)), "0 gwei");
    }

    #[test]
    fn humanize_gwei_renders_sub_gwei_as_threshold() {
        assert_eq!(humanize_gwei(w(1)), "<0.0001 gwei");
    }

    #[test]
    fn humanize_gwei_renders_integer_gwei() {
        assert_eq!(humanize_gwei(w(14 * WEI_PER_GWEI)), "14 gwei");
    }

    #[test]
    fn humanize_gwei_renders_with_decimals() {
        // 14.5 gwei.
        assert_eq!(
            humanize_gwei(w(14 * WEI_PER_GWEI + WEI_PER_GWEI / 2)),
            "14.5 gwei"
        );
    }

    #[test]
    fn humanize_gwei_truncates_to_four_decimals() {
        // 14.12345 gwei.
        let amount = 14 * WEI_PER_GWEI + 123_450_000;
        assert_eq!(humanize_gwei(w(amount)), "14.1234 gwei");
    }

    #[test]
    fn humanize_gwei_groups_thousands() {
        assert_eq!(humanize_gwei(w(12_345 * WEI_PER_GWEI)), "12,345 gwei");
    }

    #[test]
    fn humanize_gas_units_zero_and_small() {
        assert_eq!(humanize_gas_units(0), "0");
        assert_eq!(humanize_gas_units(5), "5");
        assert_eq!(humanize_gas_units(999), "999");
    }

    #[test]
    fn humanize_gas_units_groups_thousands() {
        assert_eq!(humanize_gas_units(1_000), "1,000");
        assert_eq!(humanize_gas_units(52_341), "52,341");
        assert_eq!(humanize_gas_units(21_000_000), "21,000,000");
    }
}
