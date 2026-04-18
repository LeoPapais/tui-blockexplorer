//! Contract-source metadata surfaced by Etherscan verifications.
//!
//! Only the subset we need for plan 4 decoding lands here. Full Source
//! tab content (multiple files, proxy hints, compiler metadata) arrives
//! with plan 7 when the Contract Detail tabs are expanded.
//!
//! See `plan/4-tx-detail.md` section 12.4.1.

/// ABI + minimal metadata returned by Etherscan's `getabi`/`getsourcecode`.
/// `abi` is the raw JSON the adapter stores verbatim so future use cases
/// can decode against their own parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractAbi {
    pub abi: String,
    /// `true` when Etherscan reports the contract as verified. Stays
    /// useful once the Source tab lands, but in MVP we only look at
    /// the ABI field.
    pub is_verified: bool,
}
