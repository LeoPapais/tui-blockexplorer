//! Pure helper that maps an `eth_getCode` response to an
//! [`AddressKind`]. Peels off the 23-byte EIP-7702 delegation
//! designator (`0xef0100` followed by a 20-byte delegate) before
//! falling through to the classic EOA-vs-contract rule.
//!
//! See `plan/15-backlog.md` section 3.1.

use crate::domain::{Address, AddressKind, DomainError};

/// EIP-7702 delegation designator prefix.
const DELEGATION_DESIGNATOR: &str = "ef0100";
/// Total length of a delegation designator in hex characters
/// (3 bytes prefix + 20 bytes delegate = 23 bytes = 46 hex chars).
const DELEGATION_HEX_LEN: usize = 46;

/// Classify the raw `eth_getCode` result `code` (a `0x`-prefixed hex
/// string, possibly `"0x"` for EOAs). Returns [`DomainError::Internal`]
/// when the input is not a valid hex payload — every adapter path
/// normalises to this variant so the call-site can react uniformly.
///
/// The function is pure: no I/O, no allocations beyond the borrow
/// into `code`. Exposed as a use case per `plan/15-backlog.md`
/// section 3.1 because the same logic is used by
/// `AlchemyAddressLookup::classify` and `AlchemyAddressReader::get`.
pub fn run(code: &str) -> Result<AddressKind, DomainError> {
    let body = code
        .strip_prefix("0x")
        .or_else(|| code.strip_prefix("0X"))
        .unwrap_or(code)
        .to_ascii_lowercase();

    // Empty or all-zero code slot -> plain EOA. We do not validate
    // the hex in this branch because "0x" is the canonical empty
    // body and calling `hex::decode` on it is a no-op.
    if body.is_empty() || body.chars().all(|c| c == '0') {
        return Ok(AddressKind::Eoa { delegated_to: None });
    }

    // Reject any payload whose nibbles are not valid hex. The adapter
    // layer already enforces this for most responses, but keep the
    // check here so the use case stays usable in isolation.
    if !body.len().is_multiple_of(2) || !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(DomainError::Internal(format!(
            "eth_getCode returned non-hex payload: {code}"
        )));
    }

    // EIP-7702: a valid delegation tag is exactly `0xef0100` + 20
    // bytes of delegate address. Anything shorter (even with a
    // matching prefix) is malformed and falls through to Contract so
    // we do not silently hide bytecode.
    if body.starts_with(DELEGATION_DESIGNATOR) && body.len() == DELEGATION_HEX_LEN {
        let delegate_hex = &body[DELEGATION_DESIGNATOR.len()..];
        let mut bytes = [0u8; 20];
        hex::decode_to_slice(delegate_hex, &mut bytes).map_err(|e| {
            DomainError::Internal(format!("delegation delegate is not hex: {e}"))
        })?;
        return Ok(AddressKind::Eoa {
            delegated_to: Some(Address::from_bytes(bytes)),
        });
    }

    Ok(AddressKind::Contract)
}
