//! Transaction identifiers.
//!
//! See `plan/2-search.md` section 10.1.

use crate::domain::{BlockNumber, DomainError};

/// A 32-byte transaction hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TxHash([u8; 32]);

impl TxHash {
    /// Parse a `0x`-prefixed lowercase or uppercase hex string into a
    /// `TxHash`.
    pub fn from_hex(s: &str) -> Result<Self, DomainError> {
        parse_hash32(s, "tx hash").map(Self)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(66);
        out.push_str("0x");
        out.push_str(&hex::encode(self.0));
        out
    }
}

/// Lightweight summary used by `TxLookupPort`. The full transaction
/// entity will live in `plan/4-tx-detail.md` once that screen is
/// implemented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxSummary {
    pub hash: TxHash,
    pub block: Option<BlockNumber>,
}

pub(crate) fn parse_hash32(s: &str, label: &str) -> Result<[u8; 32], DomainError> {
    let stripped = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .ok_or_else(|| DomainError::InvalidInput(format!("{label} must start with 0x: {s}")))?;
    if stripped.len() != 64 {
        return Err(DomainError::InvalidInput(format!(
            "{label} must be 32 bytes / 64 hex chars, got {}",
            stripped.len()
        )));
    }
    let mut bytes = [0u8; 32];
    hex::decode_to_slice(stripped, &mut bytes)
        .map_err(|e| DomainError::InvalidInput(format!("invalid hex: {e}")))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_lowercase_tx_hash() {
        let h = TxHash::from_hex(
            "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
        )
        .unwrap();
        assert_eq!(
            h.to_hex(),
            "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b"
        );
    }

    #[test]
    fn rejects_short_hex() {
        let err = TxHash::from_hex("0xabcd").unwrap_err();
        assert!(matches!(err, DomainError::InvalidInput(_)));
    }
}
