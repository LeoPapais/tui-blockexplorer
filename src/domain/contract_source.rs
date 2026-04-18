//! Contract-source metadata surfaced by Etherscan verifications plus
//! minimal signature hashing helpers used for ABI matching.
//!
//! - Plan 4 (`tx-detail`) needs the raw ABI string for calldata
//!   decoding, surfaced as [`ContractAbi`].
//! - Plan 7 (`contract-detail`) additionally needs file contents,
//!   compiler metadata and proxy hints; those live on
//!   [`ContractSource`].
//!
//! See `plan/4-tx-detail.md` section 12.4.1 and
//! `plan/7-contract-detail.md` section 12.4.1.

use crate::domain::Address;
use tiny_keccak::{Hasher, Keccak};

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

/// Single file making up a verified contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub path: String,
    pub content: String,
}

/// Full source + compiler metadata for the Contract Detail screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractSource {
    pub is_verified: bool,
    pub contract_name: String,
    pub compiler_version: String,
    pub optimizer_enabled: bool,
    pub optimizer_runs: u32,
    pub evm_version: String,
    pub license: String,
    pub abi: String,
    pub files: Vec<SourceFile>,
    /// Populated when Etherscan reports a proxy implementation
    /// address via `getsourcecode` (second source of truth besides
    /// EIP-1967 detection).
    pub implementation: Option<Address>,
}

/// Decode Etherscan's quirky `SourceCode` field, which ships in
/// three flavours:
///
/// 1. A raw string — single-file contract.
/// 2. A JSON object keyed by path: `{"Foo.sol":{"content":"..."}}`.
/// 3. A double-wrapped JSON: `{{"sources":{"Foo.sol":{"content":
///    "..."}},"settings":{...}}}` — this is a standard Solidity
///    compiler-input envelope with two extra braces around it.
///
/// `fallback_name` is used for flavour (1) since that flavour does
/// not tell us what the file was called.
#[must_use]
pub fn parse_etherscan_source_envelope(raw: &str, fallback_name: &str) -> Vec<SourceFile> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    // Flavour 3: strip the outer double-braces.
    if let Some(stripped) = trimmed
        .strip_prefix("{{")
        .and_then(|s| s.strip_suffix("}}"))
    {
        let unwrapped = format!("{{{stripped}}}");
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&unwrapped)
            && let Some(files) = extract_sources_map(&value)
        {
            return files;
        }
    }

    // Flavour 2: plain JSON object keyed by path.
    if trimmed.starts_with('{')
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed)
    {
        if let Some(files) = extract_sources_map(&value) {
            return files;
        }
        if let Some(files) = extract_path_to_content_map(&value) {
            return files;
        }
    }

    // Flavour 1: single-file source.
    vec![SourceFile {
        path: fallback_name.to_string(),
        content: raw.to_string(),
    }]
}

/// Accept `{"sources": {"path": {"content": "..."}}}` shaped input.
fn extract_sources_map(value: &serde_json::Value) -> Option<Vec<SourceFile>> {
    let sources = value.get("sources")?.as_object()?;
    let mut out = Vec::with_capacity(sources.len());
    for (path, entry) in sources {
        let content = entry.get("content")?.as_str()?;
        out.push(SourceFile {
            path: path.clone(),
            content: content.to_string(),
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Some(out)
}

/// Accept `{"path": {"content": "..."}}` shaped input (flavour 2).
fn extract_path_to_content_map(value: &serde_json::Value) -> Option<Vec<SourceFile>> {
    let obj = value.as_object()?;
    let mut out = Vec::with_capacity(obj.len());
    for (path, entry) in obj {
        let content = entry.get("content")?.as_str()?;
        out.push(SourceFile {
            path: path.clone(),
            content: content.to_string(),
        });
    }
    if out.is_empty() {
        return None;
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Some(out)
}

/// 4-byte function selector for a canonical signature like
/// `transfer(address,uint256)`.
#[must_use]
pub fn selector_for(signature: &str) -> [u8; 4] {
    let hash = keccak256(signature.as_bytes());
    let mut out = [0u8; 4];
    out.copy_from_slice(&hash[..4]);
    out
}

/// 32-byte event topic0 for a canonical event signature like
/// `Transfer(address,address,uint256)`.
#[must_use]
pub fn event_topic_for(signature: &str) -> [u8; 32] {
    keccak256(signature.as_bytes())
}

fn keccak256(input: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(input);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_selector_matches_known_value() {
        assert_eq!(
            selector_for("transfer(address,uint256)"),
            [0xa9, 0x05, 0x9c, 0xbb],
        );
    }

    #[test]
    fn transfer_event_topic_matches_known_value() {
        let topic = event_topic_for("Transfer(address,address,uint256)");
        let expected = hex::decode(
            "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
        )
        .unwrap();
        assert_eq!(&topic[..], &expected[..]);
    }
}
