//! Functional tests for the `load_tx_overview` use case.
//!
//! See `plan/4-tx-detail.md` section 12.1.

use blockexplorer_tui::{
    application::{SignatureSource, use_cases::load_tx_overview},
    domain::{
        Address, BlockHash, BlockNumber, Chain, ContractAbi, DomainError, LogEntry,
        Transaction, TxHash, TxStatus, TxType, Wei,
    },
};
use pretty_assertions::assert_eq;

use crate::support::stubs::{
    StubContractSourcePort, StubSignatureDirectoryPort, StubTxReaderPort,
};

fn base_tx(hash_hex: &str) -> Transaction {
    Transaction {
        chain: Chain::Ethereum,
        hash: TxHash::from_hex(hash_hex).unwrap(),
        status: TxStatus::Success,
        block_number: Some(BlockNumber::new(21_000_000)),
        block_hash: Some(
            BlockHash::from_hex(
                "0xaaaa000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
        ),
        tx_index: Some(3),
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(
            Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        ),
        value: Wei::new(0),
        gas_price: Wei::new(14_000_000_000),
        gas_used: Some(52_341),
        gas_limit: 80_000,
        nonce: 42,
        tx_type: TxType::DynamicFee,
        input: vec![0xa9, 0x05, 0x9c, 0xbb], // transfer() selector
        logs: Vec::new(),
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

    assert_eq!(got.tx, tx);
    assert!(got.decoded_method.is_none());
    assert!(got.decoded_logs.is_empty());
    assert_eq!(
        got.tx.fee_paid().expect("mined tx has fee").value(),
        14_000_000_000u128 * 52_341u128
    );
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

    match got.tx.status {
        TxStatus::Failed { reason } => {
            assert_eq!(reason.as_deref(), Some("InsufficientBalance()"));
        }
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[tokio::test]
async fn pending_tx_has_no_block_and_pending_status() {
    let reader = StubTxReaderPort::new();
    let mut tx = base_tx(
        "0xbeef016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944a",
    );
    tx.status = TxStatus::Pending;
    tx.block_number = None;
    tx.block_hash = None;
    tx.tx_index = None;
    tx.gas_used = None;
    reader.insert(tx.clone());

    let got = load_tx_overview::run(&reader, tx.hash, Chain::Ethereum)
        .await
        .expect("ok");

    assert!(got.tx.is_pending());
    assert!(got.tx.fee_paid().is_none());
    assert!(got.tx.block_number.is_none());
}

#[tokio::test]
async fn decoding_prefers_abi_over_signature_directory() {
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();

    let tx = base_tx(
        "0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa",
    );
    reader.insert(tx.clone());

    let to = tx.to.expect("base tx targets a contract");
    let abi = ContractAbi {
        abi: r#"[{"type":"function","name":"transfer","inputs":[{"name":"to","type":"address"},{"name":"value","type":"uint256"}],"outputs":[]}]"#
            .to_string(),
        is_verified: true,
    };
    contract_source.insert(to, abi);
    signatures.set_selector([0xa9, 0x05, 0x9c, 0xbb], "sigdb_override(address,uint256)");

    let got = load_tx_overview::run_with_decoding(
        &reader,
        &contract_source,
        &signatures,
        tx.hash,
        Chain::Ethereum,
    )
    .await
    .expect("ok");

    let method = got.decoded_method.expect("method decoded");
    assert_eq!(method.signature, "transfer(address,uint256)");
    assert_eq!(method.source, SignatureSource::Abi);
}

#[tokio::test]
async fn decoding_falls_back_to_signature_directory() {
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();

    let tx = base_tx(
        "0xbbbb016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394bb",
    );
    reader.insert(tx.clone());
    signatures.set_selector([0xa9, 0x05, 0x9c, 0xbb], "transfer(address,uint256)");

    let got = load_tx_overview::run_with_decoding(
        &reader,
        &contract_source,
        &signatures,
        tx.hash,
        Chain::Ethereum,
    )
    .await
    .expect("ok");

    let method = got.decoded_method.expect("method decoded");
    assert_eq!(method.signature, "transfer(address,uint256)");
    assert_eq!(method.source, SignatureSource::SignatureDirectory);
}

#[tokio::test]
async fn logs_are_decoded_via_signature_directory() {
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();

    let mut tx = base_tx(
        "0xcccc016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394cc",
    );
    let topic: [u8; 32] = hex::decode(
        "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
    )
    .unwrap()
    .try_into()
    .unwrap();
    tx.logs.push(LogEntry {
        address: Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        topics: vec![topic],
        data: Vec::new(),
    });
    reader.insert(tx.clone());
    signatures.set_event_topic(topic, "Transfer(address,address,uint256)");

    let got = load_tx_overview::run_with_decoding(
        &reader,
        &contract_source,
        &signatures,
        tx.hash,
        Chain::Ethereum,
    )
    .await
    .expect("ok");

    assert_eq!(got.decoded_logs.len(), 1);
    let sig = got.decoded_logs[0].signature.as_ref().expect("decoded");
    assert_eq!(sig.signature, "Transfer(address,address,uint256)");
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
