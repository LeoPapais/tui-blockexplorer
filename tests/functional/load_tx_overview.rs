//! Functional tests for the `load_tx_overview` use case.
//!
//! See `plan/4-tx-detail.md` section 12.1.

use blockexplorer_tui::{
    application::use_cases::load_tx_overview,
    domain::{
        Address, BlockHash, BlockNumber, Chain, DomainError, Transaction, TxHash, TxStatus,
        TxType, Wei,
    },
};
use pretty_assertions::assert_eq;

use crate::support::stubs::StubTxReaderPort;

fn base_tx(hash_hex: &str) -> Transaction {
    Transaction {
        chain: Chain::Ethereum,
        hash: TxHash::from_hex(hash_hex).unwrap(),
        status: TxStatus::Success,
        block_number: BlockNumber::new(21_000_000),
        block_hash: BlockHash::from_hex(
            "0xaaaa000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap(),
        tx_index: 3,
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(
            Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        ),
        value: Wei::new(0),
        gas_price: Wei::new(14_000_000_000),
        gas_used: 52_341,
        gas_limit: 80_000,
        nonce: 42,
        tx_type: TxType::DynamicFee,
        input: vec![0xa9, 0x05, 0x9c, 0xbb], // transfer() selector
        raw_json: "{\"raw\":\"fixture\"}".to_string(),
    }
}

#[tokio::test]
async fn returns_a_successful_tx() {
    let reader = StubTxReaderPort::new();
    let tx = base_tx(
        "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
    );
    reader.insert(tx.clone());

    let got = load_tx_overview::run(&reader, tx.hash, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got, tx);
    assert_eq!(got.fee_paid().value(), 14_000_000_000u128 * 52_341u128);
}

#[tokio::test]
async fn carries_revert_reason_on_failure() {
    let reader = StubTxReaderPort::new();
    let mut tx = base_tx(
        "0xfefe016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944f",
    );
    tx.status = TxStatus::Failed {
        reason: Some("InsufficientBalance()".into()),
    };
    reader.insert(tx.clone());

    let got = load_tx_overview::run(&reader, tx.hash, Chain::Ethereum)
        .await
        .expect("ok");

    match got.status {
        TxStatus::Failed { reason } => {
            assert_eq!(reason.as_deref(), Some("InsufficientBalance()"));
        }
        TxStatus::Success => panic!("expected Failed, got Success"),
    }
}

#[tokio::test]
async fn missing_tx_returns_not_found() {
    let reader = StubTxReaderPort::new();
    let hash = TxHash::from_hex(
        "0x0000000000000000000000000000000000000000000000000000000000000001",
    )
    .unwrap();

    let err = load_tx_overview::run(&reader, hash, Chain::Ethereum)
        .await
        .expect_err("missing tx must error");

    assert!(matches!(err, DomainError::NotFound));
}
