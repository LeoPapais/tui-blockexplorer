//! Fungible-token metadata. Minimal for now; expanded in
//! `plan/8-token-detail.md`.

use crate::domain::Address;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenMetadata {
    pub address: Address,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
}
