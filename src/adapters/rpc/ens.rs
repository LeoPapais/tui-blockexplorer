//! Alchemy-backed [`EnsResolverPort`].
//!
//! Forward resolution issues two `eth_call`s: first `resolver(node)` on
//! the ENS Registry to discover the resolver contract, then
//! `addr(node)` on that resolver to read the address record. Reverse
//! resolution is stubbed out until the UI actually renders a reverse
//! name; the forward path covers every BDD scenario in
//! `plan/2-search.md` section 6.
//!
//! Following the external-apis rule, we do not pull in any ENS SDK
//! crate; namehash and ABI encoding are implemented in house.
//! See `plan/2-search.md` section 10.2.

use tiny_keccak::{Hasher, Keccak};

use super::client::RpcClient;
use crate::{
    application::ports::EnsResolverPort,
    domain::{Address, Chain, DomainError},
};

/// ENS Registry on every EVM chain that exposes the same deployment.
/// For non-mainnet chains the registry address differs; that is out
/// of scope for this phase and treated as "no match".
const ENS_REGISTRY: &str = "0x00000000000c2e074ec69a0dfb2997ba6c7d2e1e";

/// `resolver(bytes32)` selector: keccak256("resolver(bytes32)")[0..4].
const SELECTOR_RESOLVER: [u8; 4] = [0x01, 0x78, 0xb8, 0xbf];

/// `addr(bytes32)` selector: keccak256("addr(bytes32)")[0..4].
const SELECTOR_ADDR: [u8; 4] = [0x3b, 0x3b, 0x57, 0xde];

#[derive(Debug, Clone)]
pub struct AlchemyEnsResolver {
    client: RpcClient,
}

impl AlchemyEnsResolver {
    #[must_use]
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }

    async fn call_registry_resolver(
        &self,
        node: &[u8; 32],
    ) -> Result<Option<String>, DomainError> {
        let data = encode_call(SELECTOR_RESOLVER, node);
        let resp: String = self
            .client
            .call(
                "eth_call",
                serde_json::json!([
                    { "to": ENS_REGISTRY, "data": data },
                    "latest"
                ]),
            )
            .await
            .map_err(|e| e.into_domain())?;
        Ok(extract_address_from_word(&resp))
    }

    async fn call_resolver_addr(
        &self,
        resolver: &str,
        node: &[u8; 32],
    ) -> Result<Option<Address>, DomainError> {
        let data = encode_call(SELECTOR_ADDR, node);
        let resp: String = self
            .client
            .call(
                "eth_call",
                serde_json::json!([
                    { "to": resolver, "data": data },
                    "latest"
                ]),
            )
            .await
            .map_err(|e| e.into_domain())?;
        let Some(hex_addr) = extract_address_from_word(&resp) else {
            return Ok(None);
        };
        Ok(Some(Address::from_hex(&hex_addr)?))
    }
}

impl EnsResolverPort for AlchemyEnsResolver {
    async fn forward(
        &self,
        name: &str,
        _chain: Chain,
    ) -> Result<Option<Address>, DomainError> {
        let node = namehash(name);
        let Some(resolver) = self.call_registry_resolver(&node).await? else {
            return Ok(None);
        };
        self.call_resolver_addr(&resolver, &node).await
    }

    async fn reverse(
        &self,
        _address: Address,
        _chain: Chain,
    ) -> Result<Option<String>, DomainError> {
        // Deferred: the BDD scenarios in plan/2-search.md only require
        // forward resolution. Reverse resolution needs string decoding
        // (ABI-encoded `string` return value) which is scheduled for a
        // later phase.
        Ok(None)
    }
}

/// EIP-137 namehash. Processes labels from right to left, hashing
/// each label with keccak256 before concatenating.
#[must_use]
pub fn namehash(name: &str) -> [u8; 32] {
    let mut node = [0u8; 32];
    if name.is_empty() {
        return node;
    }
    let labels: Vec<&str> = name.split('.').collect();
    for label in labels.iter().rev() {
        let label_hash = keccak256(label.as_bytes());
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(&node);
        buf[32..].copy_from_slice(&label_hash);
        node = keccak256(&buf);
    }
    node
}

fn keccak256(input: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(input);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

/// Encode `selector || bytes32_param` as a `0x`-prefixed hex string.
fn encode_call(selector: [u8; 4], param: &[u8; 32]) -> String {
    let mut buf = [0u8; 36];
    buf[..4].copy_from_slice(&selector);
    buf[4..].copy_from_slice(param);
    let mut out = String::with_capacity(2 + 72);
    out.push_str("0x");
    out.push_str(&hex::encode(buf));
    out
}

/// Extract the `0x`-prefixed address from the last 20 bytes of a
/// 32-byte return word. Returns `None` when the word is all zeros.
fn extract_address_from_word(hex_word: &str) -> Option<String> {
    let stripped = hex_word.strip_prefix("0x").unwrap_or(hex_word);
    if stripped.len() < 64 {
        return None;
    }
    let tail = &stripped[stripped.len() - 40..];
    if tail.chars().all(|c| c == '0') {
        return None;
    }
    Some(format!("0x{tail}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namehash_of_empty_string_is_zero() {
        assert_eq!(namehash(""), [0u8; 32]);
    }

    #[test]
    fn namehash_of_eth_matches_known_value() {
        // Reference: https://docs.ens.domains/contract-api-reference/name-processing#hashing-names
        let want = hex::decode(
            "93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae",
        )
        .unwrap();
        assert_eq!(namehash("eth")[..], want[..]);
    }

    #[test]
    fn extract_address_picks_last_20_bytes() {
        let word =
            "0x000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045";
        assert_eq!(
            extract_address_from_word(word).as_deref(),
            Some("0xd8da6bf26964af9d7eed9e03e53415d37aa96045")
        );
    }

    #[test]
    fn extract_address_returns_none_on_all_zero() {
        let word =
            "0x0000000000000000000000000000000000000000000000000000000000000000";
        assert_eq!(extract_address_from_word(word), None);
    }
}
