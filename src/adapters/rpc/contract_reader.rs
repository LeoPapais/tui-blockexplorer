//! Alchemy-backed [`ContractReaderPort`] adapter.
//!
//! Encodes arguments against the supplied [`AbiFunction`], issues
//! `eth_call` at `"latest"`, and decodes the returned bytes. Revert
//! data following the `Error(string)` selector `0x08c379a0` is
//! surfaced as `DomainError::ExecutionReverted { reason }`.
//!
//! The codec is deliberately minimal: it handles the static head
//! section for `uintN`, `intN`, `address`, `bool`, `bytesN`, plus
//! dynamic `string` / `bytes`. More exotic types (arrays, tuples,
//! mappings) are intentionally out of scope and caught upstream by
//! [`AbiFunction::is_executable`].
//!
//! See `plan/7-contract-detail.md` section 12.4.2.

use serde_json::json;

use super::client::{RpcClient, RpcError};
use crate::{
    application::ports::ContractReaderPort,
    domain::{
        AbiFunction, AbiParamType, AbiValue, Address, Chain, DecodedValue, DomainError,
        contract_source::selector_for,
    },
};

#[derive(Debug, Clone)]
pub struct AlchemyContractReader {
    client: RpcClient,
}

impl AlchemyContractReader {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }
}

impl ContractReaderPort for AlchemyContractReader {
    async fn call(
        &self,
        address: Address,
        _chain: Chain,
        function: &AbiFunction,
        args: Vec<AbiValue>,
    ) -> Result<Vec<DecodedValue>, DomainError> {
        let signature = function.signature();
        let selector = selector_for(&signature);
        let mut calldata = Vec::with_capacity(4 + args.len() * 32);
        calldata.extend_from_slice(&selector);
        encode_args(&function.inputs, &args, &mut calldata)?;

        let tx = json!({
            "to": address.to_hex(),
            "data": format!("0x{}", hex::encode(&calldata)),
        });

        let raw: Result<String, RpcError> =
            self.client.call("eth_call", json!([tx, "latest"])).await;

        match raw {
            Ok(hex_bytes) => {
                let bytes = decode_hex(&hex_bytes)?;
                decode_outputs(&function.outputs, &bytes)
            }
            Err(RpcError::Rpc { code: 3, message })
            | Err(RpcError::Rpc {
                code: -32000,
                message,
            })
            | Err(RpcError::Rpc {
                code: -32015,
                message,
            }) => {
                // Nodes return revert data with a variety of codes;
                // the message usually contains either the raw
                // "revert <reason>" string or just the error text.
                Err(DomainError::ExecutionReverted {
                    reason: extract_revert_reason(&message),
                })
            }
            Err(err) => Err(err.into_domain()),
        }
    }
}

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

fn encode_args(
    inputs: &[crate::domain::AbiParam],
    args: &[AbiValue],
    out: &mut Vec<u8>,
) -> Result<(), DomainError> {
    if inputs.len() != args.len() {
        return Err(DomainError::InvalidInput(format!(
            "arg count mismatch: expected {}, got {}",
            inputs.len(),
            args.len()
        )));
    }

    // Two-pass encoding:
    //   Pass 1: emit head slots for every input (32 bytes each).
    //   Pass 2: append dynamic tails (string/bytes) and back-patch
    //           the head slot with the actual offset.
    let head_start = out.len();
    let mut dynamic_slots: Vec<(usize, Vec<u8>)> = Vec::new();

    for (idx, (param, value)) in inputs.iter().zip(args.iter()).enumerate() {
        match (&param.kind, value) {
            (AbiParamType::Uint { bits }, AbiValue::Uint(v)) => {
                out.extend_from_slice(&encode_uint(*v, *bits));
            }
            (AbiParamType::Address, AbiValue::Address(addr)) => {
                out.extend_from_slice(&encode_address(*addr));
            }
            (AbiParamType::Bool, AbiValue::Bool(b)) => {
                out.extend_from_slice(&encode_uint(u128::from(*b), 256));
            }
            (AbiParamType::String, AbiValue::String(s)) => {
                let head_offset = head_start + idx * 32;
                let encoded = encode_dynamic_bytes(s.as_bytes());
                dynamic_slots.push((head_offset, encoded));
                out.extend_from_slice(&[0u8; 32]); // placeholder
            }
            (kind, value) => {
                return Err(DomainError::InvalidInput(format!(
                    "cannot encode {value:?} as {kind:?}"
                )));
            }
        }
    }

    // Pass 2: append tails, patching the head placeholders.
    let dynamic_region_start = out.len();
    for (head_offset, tail) in dynamic_slots {
        let offset_from_head = out.len() - head_start;
        let offset_bytes = encode_uint(offset_from_head as u128, 256);
        out[head_offset..head_offset + 32].copy_from_slice(&offset_bytes);
        out.extend_from_slice(&tail);
    }
    let _ = dynamic_region_start;
    Ok(())
}

fn encode_uint(value: u128, bits: u16) -> [u8; 32] {
    // All uints are right-aligned in a 32-byte word regardless of
    // bit-width.
    debug_assert!(bits <= 256);
    let mut out = [0u8; 32];
    let be = value.to_be_bytes();
    out[32 - 16..].copy_from_slice(&be);
    out
}

fn encode_address(addr: Address) -> [u8; 32] {
    let mut out = [0u8; 32];
    // Address is 20 bytes, right-aligned (12 leading zero bytes).
    out[12..].copy_from_slice(addr.as_bytes());
    out
}

fn encode_dynamic_bytes(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + data.len().next_multiple_of(32));
    out.extend_from_slice(&encode_uint(data.len() as u128, 256));
    out.extend_from_slice(data);
    let padding = (32 - data.len() % 32) % 32;
    out.extend(std::iter::repeat_n(0u8, padding));
    out
}

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

fn decode_outputs(
    outputs: &[crate::domain::AbiParam],
    data: &[u8],
) -> Result<Vec<DecodedValue>, DomainError> {
    if outputs.is_empty() {
        return Ok(Vec::new());
    }
    if data.is_empty() {
        return Err(DomainError::Internal(
            "eth_call returned empty data for a function with outputs".into(),
        ));
    }
    let mut values = Vec::with_capacity(outputs.len());
    for (idx, param) in outputs.iter().enumerate() {
        let head_offset = idx * 32;
        if head_offset + 32 > data.len() {
            return Err(DomainError::Internal(format!(
                "eth_call returndata too short: need {} bytes, got {}",
                head_offset + 32,
                data.len()
            )));
        }
        let word = &data[head_offset..head_offset + 32];
        let decoded = match &param.kind {
            AbiParamType::Uint { .. } => DecodedValue::Uint(word_to_u128(word)?),
            AbiParamType::Int { .. } => DecodedValue::Int(word_to_i128(word)?),
            AbiParamType::Address => DecodedValue::Address(word_to_address(word)?),
            AbiParamType::Bool => DecodedValue::Bool(word_to_u128(word)? == 1),
            AbiParamType::BytesN(n) => {
                let end = *n as usize;
                DecodedValue::Bytes(word[..end].to_vec())
            }
            AbiParamType::String | AbiParamType::Bytes => {
                let offset = word_to_u128(word)? as usize;
                if offset + 32 > data.len() {
                    return Err(DomainError::Internal("dynamic offset overflow".into()));
                }
                let length = word_to_u128(&data[offset..offset + 32])? as usize;
                let start = offset + 32;
                let end = start + length;
                if end > data.len() {
                    return Err(DomainError::Internal("dynamic payload overflow".into()));
                }
                if matches!(param.kind, AbiParamType::String) {
                    let text = String::from_utf8_lossy(&data[start..end]).into_owned();
                    DecodedValue::String(text)
                } else {
                    DecodedValue::Bytes(data[start..end].to_vec())
                }
            }
            AbiParamType::Unsupported(raw) => {
                return Err(DomainError::Internal(format!(
                    "unsupported output type: {raw}"
                )));
            }
        };
        values.push(decoded);
    }
    Ok(values)
}

fn word_to_u128(word: &[u8]) -> Result<u128, DomainError> {
    if word.len() != 32 {
        return Err(DomainError::Internal("word must be 32 bytes".into()));
    }
    // High 16 bytes must be zero or the number would overflow u128.
    if word[..16].iter().any(|b| *b != 0) {
        return Err(DomainError::Internal(
            "value overflows u128 representation".into(),
        ));
    }
    let mut buf = [0u8; 16];
    buf.copy_from_slice(&word[16..]);
    Ok(u128::from_be_bytes(buf))
}

fn word_to_i128(word: &[u8]) -> Result<i128, DomainError> {
    if word.len() != 32 {
        return Err(DomainError::Internal("word must be 32 bytes".into()));
    }
    // Treat the low 16 bytes as a two's-complement i128; ensure the
    // sign-extended high half agrees.
    let sign = word[0] & 0x80 != 0;
    let expected = if sign { 0xff } else { 0 };
    if word[..16].iter().any(|b| *b != expected) {
        return Err(DomainError::Internal(
            "value overflows i128 representation".into(),
        ));
    }
    let mut buf = [0u8; 16];
    buf.copy_from_slice(&word[16..]);
    Ok(i128::from_be_bytes(buf))
}

fn word_to_address(word: &[u8]) -> Result<Address, DomainError> {
    if word.len() != 32 {
        return Err(DomainError::Internal("word must be 32 bytes".into()));
    }
    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&word[12..]);
    Ok(Address::from_bytes(bytes))
}

fn decode_hex(s: &str) -> Result<Vec<u8>, DomainError> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    if stripped.is_empty() {
        return Ok(Vec::new());
    }
    hex::decode(stripped).map_err(|e| DomainError::Internal(format!("invalid hex: {e}")))
}

/// Pull the user-facing revert string out of a provider error
/// message. Nodes sometimes return the `Error(string)` payload
/// verbatim (starting with `0x08c379a0`), sometimes expose the
/// reason string directly. We try the hex path first.
fn extract_revert_reason(message: &str) -> String {
    // Typical Alchemy format:
    //   "execution reverted: InsufficientBalance()"
    if let Some(rest) = message.split_once(':').map(|(_, r)| r.trim())
        && !rest.is_empty()
    {
        return rest.to_string();
    }
    // Try to decode the hex payload if the message embeds one.
    if let Some(idx) = message.find("0x")
        && let Ok(bytes) = hex::decode(message[idx + 2..].trim_end())
        && bytes.len() >= 4
        && bytes[..4] == [0x08, 0xc3, 0x79, 0xa0]
    {
        let payload = &bytes[4..];
        if payload.len() >= 64 {
            let length = word_to_u128(&payload[32..64]).unwrap_or(0) as usize;
            if 64 + length <= payload.len() {
                return String::from_utf8_lossy(&payload[64..64 + length]).into_owned();
            }
        }
    }
    message.to_string()
}
