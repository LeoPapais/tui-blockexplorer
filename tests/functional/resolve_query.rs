//! Functional tests for `ResolveQuery`.
//!
//! See `plan/2-search.md` section 10.1.

use blockexplorer_tui::{
    application::use_cases::resolve_query::{Classification, ResolveQuery, classify, classify_input},
    domain::{
        Address, AddressKind, BlockHash, BlockNumber, BlockSummary, Chain, DomainError,
        ResolvedEntity, TokenMetadata, TxHash, TxSummary,
    },
};
use pretty_assertions::assert_eq;
use rstest::rstest;

use crate::support::stubs::{
    StubAddressLookupPort, StubBlockLookupPort, StubEnsResolverPort, StubTokenSearchPort,
    StubTxLookupPort,
};

// ---------------------------------------------------------------------------
// Fixture addresses / hashes
// ---------------------------------------------------------------------------

const TX_HEX: &str = "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b";
const BLOCK_HEX: &str = "0xabcdef0000000000000000000000000000000000000000000000000000000000";
const ADDR_HEX: &str = "0xd8da6bf26964af9d7eed9e03e53415d37aa96045";
const VITALIK_ADDR_HEX: &str = "0xd8da6bf26964af9d7eed9e03e53415d37aa96045";

fn fresh() -> (
    StubBlockLookupPort,
    StubTxLookupPort,
    StubAddressLookupPort,
    StubEnsResolverPort,
    StubTokenSearchPort,
) {
    (
        StubBlockLookupPort::new(),
        StubTxLookupPort::new(),
        StubAddressLookupPort::new(),
        StubEnsResolverPort::new(),
        StubTokenSearchPort::new(),
    )
}

fn q<'a>(
    b: &'a StubBlockLookupPort,
    t: &'a StubTxLookupPort,
    a: &'a StubAddressLookupPort,
    e: &'a StubEnsResolverPort,
    s: &'a StubTokenSearchPort,
) -> ResolveQuery<
    'a,
    StubBlockLookupPort,
    StubTxLookupPort,
    StubAddressLookupPort,
    StubEnsResolverPort,
    StubTokenSearchPort,
> {
    ResolveQuery {
        block: b,
        tx: t,
        address: a,
        ens: e,
        token: s,
    }
}

// ---------------------------------------------------------------------------
// Classification-only tests (pure function)
// ---------------------------------------------------------------------------

#[test]
fn classification_of_tx_hash() {
    let cls = classify(TX_HEX);
    assert!(matches!(cls, Classification::Hash32 { .. }));
}

#[test]
fn classification_of_address() {
    let cls = classify(ADDR_HEX);
    assert!(matches!(cls, Classification::Address { .. }));
}

#[test]
fn classification_of_block_number() {
    assert_eq!(
        classify("21345678"),
        Classification::BlockNumber(BlockNumber::new(21_345_678))
    );
}

#[test]
fn classification_of_ens_name() {
    assert_eq!(
        classify("vitalik.eth"),
        Classification::EnsName("vitalik.eth".into())
    );
}

#[test]
fn classification_of_uppercase_ens_name_is_normalised() {
    assert_eq!(
        classify("Vitalik.ETH"),
        Classification::EnsName("vitalik.eth".into())
    );
}

#[test]
fn classification_of_ticker() {
    assert_eq!(classify("USDC"), Classification::TokenTicker("USDC".into()));
}

#[test]
fn classification_of_free_text() {
    assert_eq!(
        classify("usd coin"),
        Classification::FreeText("usd coin".into())
    );
}

// ---------------------------------------------------------------------------
// classify_input — normalisation + URL paste (plan/2 §12.1, §12.2)
// ---------------------------------------------------------------------------

#[test]
fn classify_input_trims_surrounding_whitespace_and_quotes() {
    let out = classify_input("  \"21345678\" ");
    assert_eq!(out.normalised, "21345678");
    assert_eq!(
        out.classification,
        Classification::BlockNumber(BlockNumber::new(21_345_678))
    );
}

#[test]
fn classify_input_accepts_uppercase_hex_and_lowercases_it() {
    let upper = "0xD8DA6BF26964AF9D7EED9E03E53415D37AA96045";
    let out = classify_input(upper);
    assert_eq!(out.normalised, upper.to_lowercase());
    assert!(matches!(out.classification, Classification::Address { .. }));
}

#[test]
fn classify_input_preserves_ticker_casing() {
    let out = classify_input("USDC");
    assert_eq!(out.normalised, "USDC");
    assert_eq!(
        out.classification,
        Classification::TokenTicker("USDC".into()),
    );
}

#[rstest]
#[case(
    "https://etherscan.io/tx/0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
    "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
)]
#[case(
    "http://www.etherscan.io/address/0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
)]
#[case(
    "https://polygonscan.com/tx/0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
    "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
)]
#[case(
    "https://basescan.org/block/21345678",
    "21345678",
)]
#[case(
    "https://arbiscan.io/address/0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
)]
#[case(
    "https://optimistic.etherscan.io/tx/0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
    "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
)]
fn classify_input_unwraps_block_explorer_urls(#[case] input: &str, #[case] expected: &str) {
    let out = classify_input(input);
    assert_eq!(out.normalised, expected);
}

#[test]
fn classify_input_url_to_tx_yields_hash32_classification() {
    let out = classify_input(
        "https://etherscan.io/tx/0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
    );
    assert!(matches!(out.classification, Classification::Hash32 { .. }));
}

#[test]
fn classify_input_url_to_block_yields_block_number() {
    let out = classify_input("https://etherscan.io/block/21345678");
    assert_eq!(
        out.classification,
        Classification::BlockNumber(BlockNumber::new(21_345_678)),
    );
}

#[test]
fn classify_input_url_to_address_yields_address() {
    let out = classify_input(
        "https://etherscan.io/address/0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    );
    assert!(matches!(out.classification, Classification::Address { .. }));
}

#[test]
fn classify_input_url_to_unknown_path_falls_through_to_free_text() {
    let out = classify_input("https://etherscan.io/gas-tracker");
    assert_eq!(out.normalised, "https://etherscan.io/gas-tracker");
    assert!(matches!(out.classification, Classification::FreeText(_)));
}

#[test]
fn classify_input_strips_single_quotes() {
    let out = classify_input("'vitalik.eth'");
    assert_eq!(out.normalised, "vitalik.eth");
    assert_eq!(
        out.classification,
        Classification::EnsName("vitalik.eth".into()),
    );
}

// ---------------------------------------------------------------------------
// Resolution tests (happy path + failure modes)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn resolves_a_tx_hash() {
    let (b, t, a, e, s) = fresh();
    let hash = TxHash::from_hex(TX_HEX).unwrap();
    t.insert(TxSummary {
        hash,
        block: Some(BlockNumber::new(123)),
    });

    let got = q(&b, &t, &a, &e, &s)
        .run(TX_HEX, Chain::Ethereum)
        .await
        .expect("ok");

    assert!(matches!(got.first(), Some(ResolvedEntity::Tx { .. })));
}

#[tokio::test]
async fn resolves_a_block_number() {
    let (b, t, a, e, s) = fresh();
    b.insert(BlockSummary {
        number: BlockNumber::new(21_345_678),
        hash: BlockHash::from_hex(BLOCK_HEX).unwrap(),
    });

    let got = q(&b, &t, &a, &e, &s)
        .run("21345678", Chain::Ethereum)
        .await
        .expect("ok");

    assert!(matches!(got.first(), Some(ResolvedEntity::Block { .. })));
}

#[tokio::test]
async fn disambiguates_tx_and_block_with_same_64hex() {
    let (b, t, a, e, s) = fresh();
    let hash = TxHash::from_hex(BLOCK_HEX).unwrap();
    t.insert(TxSummary { hash, block: None });
    b.insert(BlockSummary {
        number: BlockNumber::new(10),
        hash: BlockHash::from_hex(BLOCK_HEX).unwrap(),
    });

    let got = q(&b, &t, &a, &e, &s)
        .run(BLOCK_HEX, Chain::Ethereum)
        .await
        .expect("ok");

    // Tx wins the primary slot, but the block is also present.
    assert!(got.len() >= 2);
    assert!(matches!(got[0], ResolvedEntity::Tx { .. }));
    assert!(matches!(got[1], ResolvedEntity::Block { .. }));
}

#[tokio::test]
async fn resolves_ens_name_and_classifies_resulting_address() {
    let (b, t, a, e, s) = fresh();
    let addr = Address::from_hex(VITALIK_ADDR_HEX).unwrap();
    e.set_forward("vitalik.eth", addr);
    a.set_kind(addr, AddressKind::Eoa { delegated_to: None });

    let got = q(&b, &t, &a, &e, &s)
        .run("vitalik.eth", Chain::Ethereum)
        .await
        .expect("ok");

    let first = got.first().expect("at least one candidate");
    match first {
        ResolvedEntity::Address {
            address,
            kind,
            ens_name,
        } => {
            assert_eq!(*address, addr);
            assert_eq!(*kind, AddressKind::Eoa { delegated_to: None });
            assert_eq!(ens_name.as_deref(), Some("vitalik.eth"));
        }
        other => panic!("expected Address, got {other:?}"),
    }
}

#[tokio::test]
async fn classifies_address_and_includes_reverse_ens() {
    let (b, t, a, e, s) = fresh();
    let addr = Address::from_hex(ADDR_HEX).unwrap();
    a.set_kind(addr, AddressKind::Eoa { delegated_to: None });
    e.set_reverse(addr, "vitalik.eth");

    let got = q(&b, &t, &a, &e, &s)
        .run(ADDR_HEX, Chain::Ethereum)
        .await
        .expect("ok");

    let first = got.first().expect("candidate");
    match first {
        ResolvedEntity::Address { ens_name, .. } => {
            assert_eq!(ens_name.as_deref(), Some("vitalik.eth"));
        }
        other => panic!("expected Address, got {other:?}"),
    }
}

#[tokio::test]
async fn contract_address_reports_contract_kind() {
    let (b, t, a, e, s) = fresh();
    let addr = Address::from_hex(ADDR_HEX).unwrap();
    a.set_kind(addr, AddressKind::Contract);

    let got = q(&b, &t, &a, &e, &s)
        .run(ADDR_HEX, Chain::Ethereum)
        .await
        .expect("ok");

    match got.first().unwrap() {
        ResolvedEntity::Address { kind, .. } => assert_eq!(*kind, AddressKind::Contract),
        _ => panic!("expected Address"),
    }
}

#[tokio::test]
async fn contract_address_also_emits_contract_shortcut_after_the_address_row() {
    let (b, t, a, e, s) = fresh();
    let addr = Address::from_hex(ADDR_HEX).unwrap();
    a.set_kind(addr, AddressKind::Contract);

    let got = q(&b, &t, &a, &e, &s)
        .run(ADDR_HEX, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got.len(), 2, "expected Address + Contract rows");
    match &got[0] {
        ResolvedEntity::Address { kind, .. } => assert_eq!(*kind, AddressKind::Contract),
        other => panic!("expected Address first, got {other:?}"),
    }
    match &got[1] {
        ResolvedEntity::Contract { address } => assert_eq!(*address, addr),
        other => panic!("expected Contract shortcut second, got {other:?}"),
    }
}

#[tokio::test]
async fn eoa_address_does_not_emit_contract_shortcut() {
    let (b, t, a, e, s) = fresh();
    let addr = Address::from_hex(ADDR_HEX).unwrap();
    a.set_kind(addr, AddressKind::Eoa { delegated_to: None });

    let got = q(&b, &t, &a, &e, &s)
        .run(ADDR_HEX, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got.len(), 1, "EOA must not produce a Contract shortcut");
    assert!(
        !got.iter()
            .any(|c| matches!(c, ResolvedEntity::Contract { .. })),
        "EOA result must not include a Contract entry",
    );
}

#[tokio::test]
async fn delegated_eoa_emits_delegated_eoa_row_without_contract_shortcut() {
    let (b, t, a, e, s) = fresh();
    let addr = Address::from_hex(ADDR_HEX).unwrap();
    let delegate = Address::from_hex("0xc0ffee000000000000000000000000000000babe").unwrap();
    a.set_kind(
        addr,
        AddressKind::Eoa {
            delegated_to: Some(delegate),
        },
    );

    let got = q(&b, &t, &a, &e, &s)
        .run(ADDR_HEX, Chain::Ethereum)
        .await
        .expect("ok");

    // Exactly one row, a DelegatedEoa (no Contract shortcut).
    assert_eq!(got.len(), 1, "delegated EOA must emit a single row");
    match &got[0] {
        ResolvedEntity::DelegatedEoa {
            address,
            delegated_to,
        } => {
            assert_eq!(*address, addr);
            assert_eq!(*delegated_to, delegate);
        }
        other => panic!("expected DelegatedEoa, got {other:?}"),
    }
    assert!(
        !got.iter()
            .any(|c| matches!(c, ResolvedEntity::Contract { .. })),
        "delegated EOA must not include a Contract shortcut",
    );
}

#[tokio::test]
async fn ens_that_resolves_to_contract_also_emits_contract_shortcut() {
    let (b, t, a, e, s) = fresh();
    let addr = Address::from_hex(ADDR_HEX).unwrap();
    e.set_forward("usdc.eth", addr);
    a.set_kind(addr, AddressKind::Contract);

    let got = q(&b, &t, &a, &e, &s)
        .run("usdc.eth", Chain::Ethereum)
        .await
        .expect("ok");

    assert!(
        matches!(got.first(), Some(ResolvedEntity::Address { .. })),
        "Address row must still be first",
    );
    assert!(
        matches!(got.get(1), Some(ResolvedEntity::Contract { address: a }) if *a == addr),
        "Contract shortcut must follow for contract-resolved ENS",
    );
}

#[tokio::test]
async fn token_ticker_returns_token_candidates() {
    let (b, t, a, e, s) = fresh();
    let meta = TokenMetadata {
        address: Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        symbol: "USDC".into(),
        name: "USD Coin".into(),
        decimals: 6,
    };
    s.set_symbol("USDC", vec![meta.clone()]);

    let got = q(&b, &t, &a, &e, &s)
        .run("USDC", Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got.len(), 1);
    assert!(matches!(&got[0], ResolvedEntity::Token(m) if m.symbol == "USDC"));
}

#[tokio::test]
async fn free_text_searches_by_name() {
    let (b, t, a, e, s) = fresh();
    let meta = TokenMetadata {
        address: Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        symbol: "USDC".into(),
        name: "USD Coin".into(),
        decimals: 6,
    };
    s.set_name("usd coin", vec![meta.clone()]);

    let got = q(&b, &t, &a, &e, &s)
        .run("usd coin", Chain::Ethereum)
        .await
        .expect("ok");

    assert!(matches!(&got[0], ResolvedEntity::Token(m) if m.name == "USD Coin"));
}

#[tokio::test]
async fn reports_not_found_when_every_lookup_returns_nothing() {
    let (b, t, a, e, s) = fresh();

    let got = q(&b, &t, &a, &e, &s)
        .run("99999999999", Chain::Ethereum)
        .await
        .expect("ok");

    match got.first() {
        Some(ResolvedEntity::NotFound { reason }) => {
            assert!(reason.contains("Ethereum"));
            assert!(reason.contains("99999999999"));
        }
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[tokio::test]
async fn empty_input_is_rejected() {
    let (b, t, a, e, s) = fresh();

    let err = q(&b, &t, &a, &e, &s)
        .run("   ", Chain::Ethereum)
        .await
        .expect_err("whitespace must be rejected");

    assert!(matches!(err, DomainError::InvalidInput(_)));
}
