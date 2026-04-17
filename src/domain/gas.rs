//! Gas-related value objects: `Wei` and `Gwei` newtypes.
//!
//! The domain never leaks a plain `u128` to callers.

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
}
