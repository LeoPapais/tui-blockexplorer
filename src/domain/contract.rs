//! Contract-specific value objects: proxy detection metadata and the
//! ContractOverview consumed by the Contract Detail screen.
//!
//! See `plan/7-contract-detail.md` section 12.1.

use crate::domain::{Address, AddressOverview};

/// Proxy pattern recognised for a contract. Only EIP-1967 ships in
/// the MVP; transparent and UUPS variants land later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyKind {
    Eip1967,
}

impl ProxyKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ProxyKind::Eip1967 => "EIP-1967",
        }
    }
}

/// Proxy information surfaced on the Contract Detail header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProxyInfo {
    pub kind: ProxyKind,
    pub implementation: Address,
}

/// View-model used by the Contract Detail Overview tab. Reuses the
/// AddressOverview shape from plan 6 so the account header stays
/// consistent between the two detail screens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractOverview {
    pub account: AddressOverview,
    pub proxy: Option<ProxyInfo>,
}
