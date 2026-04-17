//! Transaction value objects and the full `Transaction` entity.
//!
//! See `plan/2-search.md` section 10.1 (summary) and
//! `plan/4-tx-detail.md` section 12.1 (detail).

use crate::domain::{Address, BlockHash, BlockNumber, Chain, DomainError, Wei};

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

/// Lightweight summary used by `TxLookupPort`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxSummary {
    pub hash: TxHash,
    pub block: Option<BlockNumber>,
}

/// Execution status of a mined transaction. Pending txs are not
/// modelled in this phase; when pending support lands a third variant
/// will be added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxStatus {
    Success,
    Failed { reason: Option<String> },
}

/// Transaction envelope kind, mirroring the EIP chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxType {
    /// Pre-EIP-2718 legacy transaction (type `0x0`).
    Legacy,
    /// EIP-2930 access-list transaction (type `0x1`).
    AccessList,
    /// EIP-1559 dynamic-fee transaction (type `0x2`).
    DynamicFee,
    /// EIP-4844 blob transaction (type `0x3`).
    Blob,
}

impl TxType {
    /// Parse the `0x`-prefixed `type` field of a JSON-RPC tx response.
    /// Unknown or missing values default to [`TxType::Legacy`] so the
    /// UI can still render.
    #[must_use]
    pub fn from_hex(s: &str) -> Self {
        let stripped = s.strip_prefix("0x").unwrap_or(s);
        match u8::from_str_radix(stripped, 16).unwrap_or(0) {
            0 => TxType::Legacy,
            1 => TxType::AccessList,
            2 => TxType::DynamicFee,
            3 => TxType::Blob,
            _ => TxType::Legacy,
        }
    }

    /// Human-readable label used on the Overview tab.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            TxType::Legacy => "Legacy",
            TxType::AccessList => "EIP-2930",
            TxType::DynamicFee => "EIP-1559",
            TxType::Blob => "EIP-4844",
        }
    }
}

/// Full transaction detail required by the TxDetail screen.
/// See `plan/4-tx-detail.md` section 12.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub chain: Chain,
    pub hash: TxHash,
    pub status: TxStatus,
    pub block_number: BlockNumber,
    pub block_hash: BlockHash,
    pub tx_index: u64,
    pub from: Address,
    /// `None` indicates a contract-creation tx.
    pub to: Option<Address>,
    pub value: Wei,
    pub gas_price: Wei,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub nonce: u64,
    pub tx_type: TxType,
    pub input: Vec<u8>,
    /// Raw adapter response serialized back to pretty-printed JSON.
    /// Backs the Raw tab on the TxDetail screen.
    pub raw_json: String,
}

impl Transaction {
    /// Effective fee paid in wei (`gas_used * gas_price`).
    #[must_use]
    pub fn fee_paid(&self) -> Wei {
        Wei::new(
            self.gas_price
                .value()
                .saturating_mul(u128::from(self.gas_used)),
        )
    }
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
