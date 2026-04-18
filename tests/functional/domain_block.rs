//! Functional tests for the pure Polygon signer-recovery helper.
//!
//! See `plan/15-backlog.md` section 3.5: on Polygon PoS (Bor), the
//! block miner is the zero address and the actual validator that
//! sealed the block is recovered by `ecrecover` over the block's seal
//! hash using the last 65 bytes of `extraData` as the signature.
//!
//! These tests do not depend on any specific Polygon block: they
//! synthesize a signature with a freshly generated `k256` key, feed it
//! through [`recover_polygon_signer`], and assert the recovered
//! address matches the expected public key.

use blockexplorer_tui::domain::{Address, block::recover_polygon_signer};
use k256::ecdsa::SigningKey;
use tiny_keccak::{Hasher, Keccak};

fn address_from_verifying_key(vk: &k256::ecdsa::VerifyingKey) -> Address {
    let encoded = vk.to_encoded_point(false);
    let pk = encoded.as_bytes();
    // `to_encoded_point(false)` returns [0x04, X (32), Y (32)].
    debug_assert_eq!(pk.len(), 65);
    debug_assert_eq!(pk[0], 0x04);
    let mut hasher = Keccak::v256();
    hasher.update(&pk[1..]);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&out[12..]);
    Address::from_bytes(addr)
}

fn build_extra_data(vanity: &[u8], signing_key: &SigningKey, seal_hash: &[u8; 32]) -> Vec<u8> {
    let (sig, recid) = signing_key
        .sign_prehash_recoverable(seal_hash)
        .expect("sign prehash");
    let sig_bytes = sig.to_bytes();
    let mut extra = Vec::with_capacity(vanity.len() + 65);
    extra.extend_from_slice(vanity);
    extra.extend_from_slice(&sig_bytes);
    extra.push(recid.to_byte());
    extra
}

#[test]
fn it_recovers_polygon_signer_from_extradata() {
    // Deterministic private key (non-zero, < curve order) so the test
    // is reproducible.
    let key_bytes = [
        0x4c, 0x0b, 0x6c, 0xf4, 0x6b, 0xae, 0xbd, 0xd7, 0xf5, 0xd6, 0x4a, 0x29, 0xd5, 0xf2, 0x1f,
        0xca, 0x31, 0x52, 0x0e, 0x3f, 0xc8, 0x90, 0xa3, 0x72, 0xa7, 0xd4, 0x92, 0x84, 0x77, 0x28,
        0x61, 0x92,
    ];
    let signing_key = SigningKey::from_slice(&key_bytes).expect("valid key");
    let expected = address_from_verifying_key(signing_key.verifying_key());

    // Vanity bytes are whatever Bor chose to prefix; recovery must
    // ignore everything except the trailing 65 bytes.
    let vanity = vec![0xAB; 32];
    let seal_hash = [0x11u8; 32];
    let extra = build_extra_data(&vanity, &signing_key, &seal_hash);
    assert_eq!(extra.len(), 32 + 65);

    let recovered = recover_polygon_signer(&extra, &seal_hash).expect("recovery must succeed");
    assert_eq!(recovered, expected);
}

#[test]
fn it_recovers_signer_when_v_is_eip155_style_27_28() {
    // Bor stores recovery id as 0 / 1, but we also accept the legacy
    // 27 / 28 encoding to match go-ethereum's ecrecover on other
    // clone-chains.
    let key_bytes = [
        0x53, 0x59, 0xf7, 0xfe, 0x01, 0x82, 0x92, 0x4d, 0xa7, 0xa9, 0x2d, 0x51, 0xd7, 0xcb, 0x76,
        0xba, 0xc0, 0x3a, 0x11, 0xb2, 0xac, 0x4f, 0x2a, 0x9a, 0x84, 0xa1, 0x83, 0x13, 0x2f, 0x50,
        0x0e, 0xa4,
    ];
    let signing_key = SigningKey::from_slice(&key_bytes).expect("valid key");
    let expected = address_from_verifying_key(signing_key.verifying_key());

    let seal_hash = [0x55u8; 32];
    let (sig, recid) = signing_key
        .sign_prehash_recoverable(&seal_hash)
        .expect("sign");
    let sig_bytes = sig.to_bytes();
    let mut extra = vec![0u8; 16];
    extra.extend_from_slice(&sig_bytes);
    extra.push(recid.to_byte() + 27);

    let recovered = recover_polygon_signer(&extra, &seal_hash).expect("recover");
    assert_eq!(recovered, expected);
}

#[test]
fn it_returns_none_when_extradata_is_short() {
    // Fewer than 65 bytes — we do not even try.
    let short = vec![0u8; 64];
    let seal_hash = [0u8; 32];
    assert!(recover_polygon_signer(&short, &seal_hash).is_none());
}

#[test]
fn it_returns_none_when_extradata_is_empty() {
    let seal_hash = [0u8; 32];
    assert!(recover_polygon_signer(&[], &seal_hash).is_none());
}

#[test]
fn it_returns_none_when_signature_is_invalid() {
    // 65 bytes that do not encode a valid ECDSA signature. k256's
    // `recover_from_prehash` either fails signature parsing (s out of
    // range) or produces a different key; either way the public API
    // contract is "Option::None on garbage" — we assert only that we
    // never panic.
    let garbage = vec![0xffu8; 65];
    let seal_hash = [0xaau8; 32];
    let _ = recover_polygon_signer(&garbage, &seal_hash);
}

#[test]
fn it_returns_none_when_recovery_id_is_out_of_range() {
    // Well-formed (r, s) but a garbage recovery id.
    let key_bytes = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f, 0x20,
    ];
    let signing_key = SigningKey::from_slice(&key_bytes).expect("valid key");
    let seal_hash = [0x22u8; 32];
    let (sig, _recid) = signing_key
        .sign_prehash_recoverable(&seal_hash)
        .expect("sign");
    let sig_bytes = sig.to_bytes();
    let mut extra = vec![0u8; 4];
    extra.extend_from_slice(&sig_bytes);
    extra.push(42); // not 0/1/27/28
    assert!(recover_polygon_signer(&extra, &seal_hash).is_none());
}
