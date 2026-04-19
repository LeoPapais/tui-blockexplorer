//! Compute-unit cost hints for Alchemy-flavoured JSON-RPC methods.
//!
//! MVP groundwork. The CU numbers come from Alchemy's published
//! compute-unit table
//! (<https://docs.alchemy.com/reference/compute-unit-costs>); methods
//! not in the table fall back to [`CostKind::Unknown`] with a
//! neutral weight so accounting stays non-zero without making up
//! authoritative figures. See `plan/13-alchemy-adapter.md` §8.4.

/// Kind of cost hint — used by the UI (future) to colour a method
/// that the registry does not know about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostKind {
    /// Cost sourced from Alchemy's public CU table.
    Tabulated,
    /// Cost not known to the registry; `compute_units` is a
    /// conservative placeholder (`100`).
    Unknown,
}

/// Compute-unit hint for a single RPC method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostHint {
    pub compute_units: u32,
    pub kind: CostKind,
}

impl CostHint {
    /// Build a tabulated hint (method is known to the registry).
    #[must_use]
    pub const fn tabulated(compute_units: u32) -> Self {
        Self {
            compute_units,
            kind: CostKind::Tabulated,
        }
    }

    /// Neutral fallback used for unknown methods.
    pub const UNKNOWN: Self = Self {
        compute_units: 100,
        kind: CostKind::Unknown,
    };
}

/// Return the cost hint associated with `method`. Unknown methods
/// fall back to [`CostHint::UNKNOWN`] so the caller (typically the
/// `CostMeter` in `infra`) never has to handle a missing entry.
///
/// Values are ripped from Alchemy's public CU table; see the module
/// doc comment. Keep this match narrow: only the methods the crate
/// actually calls need to live here.
#[must_use]
pub fn cost_hint_for(method: &str) -> CostHint {
    match method {
        // Node API
        "eth_blockNumber" => CostHint::tabulated(10),
        "eth_gasPrice" => CostHint::tabulated(10),
        "eth_maxPriorityFeePerGas" => CostHint::tabulated(10),
        "eth_chainId" => CostHint::tabulated(10),
        "eth_getBalance" => CostHint::tabulated(19),
        "eth_getCode" => CostHint::tabulated(19),
        "eth_getStorageAt" => CostHint::tabulated(17),
        "eth_getTransactionCount" => CostHint::tabulated(26),
        "eth_call" => CostHint::tabulated(26),
        "eth_feeHistory" => CostHint::tabulated(150),
        "eth_getBlockByNumber" | "eth_getBlockByHash" => CostHint::tabulated(16),
        "eth_getTransactionByHash" => CostHint::tabulated(17),
        "eth_getTransactionReceipt" => CostHint::tabulated(15),
        "eth_getBlockReceipts" => CostHint::tabulated(80),
        "eth_getLogs" => CostHint::tabulated(75),
        "eth_sendRawTransaction" => CostHint::tabulated(250),
        "eth_subscribe" | "eth_unsubscribe" => CostHint::tabulated(10),

        // Data API
        "alchemy_getTokenBalances" => CostHint::tabulated(37),
        "alchemy_getTokenMetadata" => CostHint::tabulated(4),
        "alchemy_getAssetTransfers" => CostHint::tabulated(150),

        // Trace / Debug namespaces
        "trace_transaction" | "trace_replayTransaction" => CostHint::tabulated(309),
        "debug_traceTransaction" => CostHint::tabulated(309),

        // Simulation API
        "alchemy_simulateAssetChanges" => CostHint::tabulated(100),

        _ => CostHint::UNKNOWN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_tabulated_hint_for_known_method() {
        let hint = cost_hint_for("eth_getBlockByNumber");
        assert_eq!(hint.compute_units, 16);
        assert_eq!(hint.kind, CostKind::Tabulated);
    }

    #[test]
    fn returns_unknown_hint_for_unlisted_method() {
        let hint = cost_hint_for("eth_not_a_real_method");
        assert_eq!(hint, CostHint::UNKNOWN);
        assert_eq!(hint.kind, CostKind::Unknown);
    }

    #[test]
    fn fallback_compute_units_is_non_zero_so_accounting_does_not_silently_skip() {
        const _: () = assert!(CostHint::UNKNOWN.compute_units > 0);
        // Runtime assertion kept as a smoke check that the const
        // path is exercised at all.
        assert_ne!(CostHint::UNKNOWN.compute_units, 0);
    }
}
