//! 20-byte account identifier shared by EOAs and contracts plus the
//! account overview consumed by the Address Detail screen.
//!
//! See `plan/2-search.md` section 10.1 and `plan/6-address-detail.md`
//! section 12.1.

use crate::domain::{AddressKind, Chain, DomainError, Wei};

/// An Ethereum-style address. Stored as the raw 20 bytes and exposed
/// through lowercase `0x`-prefixed hex for human display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Address([u8; 20]);

impl Address {
    /// Build an `Address` from a `0x`-prefixed lowercase or uppercase
    /// hex string. Case is ignored; the stored bytes are canonical.
    pub fn from_hex(s: &str) -> Result<Self, DomainError> {
        let stripped = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .ok_or_else(|| DomainError::InvalidInput(format!("address must start with 0x: {s}")))?;
        if stripped.len() != 40 {
            return Err(DomainError::InvalidInput(format!(
                "address must be 20 bytes / 40 hex chars, got {}",
                stripped.len()
            )));
        }
        let mut bytes = [0u8; 20];
        hex::decode_to_slice(stripped, &mut bytes)
            .map_err(|e| DomainError::InvalidInput(format!("invalid hex: {e}")))?;
        Ok(Self(bytes))
    }

    /// Raw bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    /// Lowercase `0x`-prefixed hex. 42 characters total.
    #[must_use]
    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(42);
        out.push_str("0x");
        out.push_str(&hex::encode(self.0));
        out
    }
}

/// Snapshot of an account shown on the Address Detail Overview tab.
/// See `plan/6-address-detail.md` section 12.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressOverview {
    pub chain: Chain,
    pub address: Address,
    pub balance: Wei,
    pub nonce: u64,
    pub kind: AddressKind,
    /// Populated when a reverse-ENS lookup succeeds. Stays `None`
    /// until the reverse-resolver adapter lands.
    pub ens_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_lowercase_hex_address() {
        let a = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
        assert_eq!(
            a.to_hex(),
            "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
        );
    }

    #[test]
    fn parses_an_uppercase_hex_address() {
        let a = Address::from_hex("0xD8DA6BF26964AF9D7EED9E03E53415D37AA96045").unwrap();
        assert_eq!(
            a.to_hex(),
            "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
        );
    }

    #[test]
    fn rejects_missing_prefix() {
        let err = Address::from_hex("d8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap_err();
        assert!(matches!(err, DomainError::InvalidInput(_)));
    }

    #[test]
    fn rejects_wrong_length() {
        let err = Address::from_hex("0xabcd").unwrap_err();
        assert!(matches!(err, DomainError::InvalidInput(_)));
    }
}
