//! Contract-specific value objects: proxy detection metadata and the
//! ContractOverview consumed by the Contract Detail screen.
//!
//! See `plan/7-contract-detail.md` sections 12.1 and 12.5.

use crate::domain::{Address, AddressOverview};

/// Proxy pattern recognised for a contract. See
/// `plan/7-contract-detail.md` section 12.5.2 for the detection
/// rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyKind {
    /// EIP-1967 transparent or generic proxy — implementation is
    /// stored in the canonical implementation slot.
    Eip1967,
    /// EIP-1822 UUPS proxy — implementation is stored at the
    /// PROXIABLE slot inside the implementation itself.
    Uups,
    /// OpenZeppelin Transparent proxy — admin slot is populated
    /// and used as the detection signal. The `implementation`
    /// field holds the admin address until the detector is split
    /// into separate impl + admin reads (see plan/7 §13).
    Transparent,
}

impl ProxyKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ProxyKind::Eip1967 => "EIP-1967",
            ProxyKind::Uups => "UUPS",
            ProxyKind::Transparent => "Transparent",
        }
    }
}

/// Where the proxy information came from. The Contract Detail UI
/// renders the source inline so the user can tell slot-based
/// detection (authoritative) from Etherscan-hinted detection
/// (provider-driven). See `plan/7-contract-detail.md` section
/// 12.5.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxySource {
    /// EIP-1967 implementation slot
    /// `0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc`.
    Eip1967Slot,
    /// EIP-1822 PROXIABLE slot
    /// `0xc5f16f0fcc639fa48a6947836d9850f504798523bf8c9a3a87d5876cf622bcf7`.
    Eip1822Slot,
    /// OpenZeppelin Transparent admin slot
    /// `0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103`.
    TransparentSlot,
    /// Etherscan `getsourcecode` response's `Implementation` field
    /// — used as a fallback when every slot reads zero.
    EtherscanHint,
}

impl ProxySource {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ProxySource::Eip1967Slot => "EIP-1967 slot",
            ProxySource::Eip1822Slot => "EIP-1822 PROXIABLE slot",
            ProxySource::TransparentSlot => "Transparent admin slot",
            ProxySource::EtherscanHint => "Etherscan",
        }
    }
}

/// Proxy information surfaced on the Contract Detail header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProxyInfo {
    pub kind: ProxyKind,
    pub implementation: Address,
    pub source: ProxySource,
}

impl ProxyInfo {
    /// Convenience constructor for the EIP-1967 slot path.
    #[must_use]
    pub const fn eip1967_slot(implementation: Address) -> Self {
        Self {
            kind: ProxyKind::Eip1967,
            implementation,
            source: ProxySource::Eip1967Slot,
        }
    }

    /// Convenience constructor for the EIP-1822 / UUPS slot path.
    #[must_use]
    pub const fn uups_slot(implementation: Address) -> Self {
        Self {
            kind: ProxyKind::Uups,
            implementation,
            source: ProxySource::Eip1822Slot,
        }
    }

    /// Convenience constructor for the Transparent admin slot path.
    /// The stored `implementation` is actually the admin address
    /// until plan/7 §13 splits it into its own field.
    #[must_use]
    pub const fn transparent_admin_slot(admin: Address) -> Self {
        Self {
            kind: ProxyKind::Transparent,
            implementation: admin,
            source: ProxySource::TransparentSlot,
        }
    }

    /// Convenience constructor for the Etherscan hint fallback —
    /// we pin the kind to `Eip1967` because Etherscan's
    /// `Implementation` field does not distinguish between proxy
    /// patterns.
    #[must_use]
    pub const fn etherscan_hint(implementation: Address) -> Self {
        Self {
            kind: ProxyKind::Eip1967,
            implementation,
            source: ProxySource::EtherscanHint,
        }
    }
}

/// View-model used by the Contract Detail Overview tab. Reuses the
/// AddressOverview shape from plan 6 so the account header stays
/// consistent between the two detail screens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractOverview {
    pub account: AddressOverview,
    pub proxy: Option<ProxyInfo>,
}
