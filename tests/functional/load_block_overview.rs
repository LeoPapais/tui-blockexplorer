//! Functional tests for the `load_block_overview` use case.
//!
//! See `plan/3-block-detail.md` section 11.1.

use blockexplorer_tui::{
    application::use_cases::load_block_overview,
    domain::{
        Address, Block, BlockHash, BlockId, BlockNumber, Chain, DomainError, Label, TxHash,
        UnixTimestamp, Wei,
    },
};
use pretty_assertions::assert_eq;

use crate::support::stubs::{StubBlockReaderPort, StubLabelPort};

fn sample_block(number: u64, hash_hex: &str) -> Block {
    let hash = BlockHash::from_hex(hash_hex).unwrap();
    let parent =
        BlockHash::from_hex("0x0000000000000000000000000000000000000000000000000000000000000000")
            .unwrap();
    let miner = Address::from_hex("0x1111111111111111111111111111111111111111").unwrap();
    let tx_hashes = vec![
        TxHash::from_hex("0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b")
            .unwrap(),
    ];
    Block {
        chain: Chain::Ethereum,
        number: BlockNumber::new(number),
        hash,
        parent_hash: parent,
        timestamp: UnixTimestamp::from_seconds(1_710_000_000),
        miner,
        gas_used: 12_000_000,
        gas_limit: 30_000_000,
        base_fee: Some(Wei::new(11_400_000_000)),
        size: 102_400,
        extra_data: vec![0x42, 0x42],
        tx_hashes,
        extra_signer: None,
    }
}

#[tokio::test]
async fn happy_path_by_number() {
    let reader = StubBlockReaderPort::new();
    let block = sample_block(
        21_345_678,
        "0xaaaa000000000000000000000000000000000000000000000000000000000000",
    );
    reader.insert(block.clone());

    let got = load_block_overview::run(&reader, BlockId::Number(block.number), Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got, block);
}

#[tokio::test]
async fn happy_path_by_hash() {
    let reader = StubBlockReaderPort::new();
    let block = sample_block(
        21_345_678,
        "0xbbbb000000000000000000000000000000000000000000000000000000000000",
    );
    reader.insert(block.clone());

    let got = load_block_overview::run(&reader, BlockId::Hash(block.hash), Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got, block);
}

#[tokio::test]
async fn missing_block_returns_not_found() {
    let reader = StubBlockReaderPort::new();

    let err = load_block_overview::run(
        &reader,
        BlockId::Number(BlockNumber::new(99_999_999)),
        Chain::Ethereum,
    )
    .await
    .expect_err("missing block must error");

    assert!(matches!(err, DomainError::NotFound));
}

#[tokio::test]
async fn multiple_blocks_are_indexed_independently() {
    let reader = StubBlockReaderPort::new();
    let a = sample_block(
        100,
        "0xa100000000000000000000000000000000000000000000000000000000000000",
    );
    let b = sample_block(
        200,
        "0xb200000000000000000000000000000000000000000000000000000000000000",
    );
    reader.insert(a.clone());
    reader.insert(b.clone());

    let got_a = load_block_overview::run(&reader, BlockId::Number(a.number), Chain::Ethereum)
        .await
        .unwrap();
    let got_b = load_block_overview::run(&reader, BlockId::Hash(b.hash), Chain::Ethereum)
        .await
        .unwrap();

    assert_eq!(got_a, a);
    assert_eq!(got_b, b);
}

// ---------------------------------------------------------------------------
// run_with_labels (plan/3 §12.4)
// ---------------------------------------------------------------------------

fn polygon_block_with_signer() -> Block {
    let hash =
        BlockHash::from_hex("0x1111111111111111111111111111111111111111111111111111111111111111")
            .unwrap();
    let parent =
        BlockHash::from_hex("0x2222222222222222222222222222222222222222222222222222222222222222")
            .unwrap();
    let miner = Address::from_hex("0x0000000000000000000000000000000000000000").unwrap();
    let signer = Address::from_hex("0x00856730088a5c3191bd26eb482e45229555ce57").unwrap();
    Block {
        chain: Chain::Polygon,
        number: BlockNumber::new(85_696_170),
        hash,
        parent_hash: parent,
        timestamp: UnixTimestamp::from_seconds(1_745_000_000),
        miner,
        gas_used: 12_000_000,
        gas_limit: 30_000_000,
        base_fee: Some(Wei::new(40_000_000_000)),
        size: 12_800,
        extra_data: vec![0u8; 32 + 65],
        tx_hashes: vec![],
        extra_signer: Some(signer),
    }
}

#[tokio::test]
async fn labels_populated_for_miner_and_signer() {
    let reader = StubBlockReaderPort::new();
    let block = polygon_block_with_signer();
    reader.insert(block.clone());

    let labels = StubLabelPort::new();
    labels.set_label(block.miner, Label::etherscan("Zero address"));
    labels.set_label(
        block.extra_signer.unwrap(),
        Label::well_known("Polygon: Validator 1"),
    );

    let view = load_block_overview::run_with_labels(
        &reader,
        &labels,
        BlockId::Number(block.number),
        Chain::Polygon,
    )
    .await
    .expect("ok");

    assert_eq!(view.block, block);
    assert_eq!(view.miner_label, Some(Label::etherscan("Zero address")));
    assert_eq!(
        view.signer_label,
        Some(Label::well_known("Polygon: Validator 1"))
    );
}

#[tokio::test]
async fn missing_labels_degrade_to_none() {
    let reader = StubBlockReaderPort::new();
    let block = sample_block(
        500,
        "0xc500000000000000000000000000000000000000000000000000000000000000",
    );
    reader.insert(block.clone());
    let labels = StubLabelPort::new();

    let view = load_block_overview::run_with_labels(
        &reader,
        &labels,
        BlockId::Number(block.number),
        Chain::Ethereum,
    )
    .await
    .expect("ok");

    assert_eq!(view.miner_label, None);
    assert_eq!(view.signer_label, None);
}

#[tokio::test]
async fn signer_label_is_skipped_when_extra_signer_is_none() {
    let reader = StubBlockReaderPort::new();
    let block = sample_block(
        501,
        "0xc501000000000000000000000000000000000000000000000000000000000000",
    );
    reader.insert(block.clone());
    let labels = StubLabelPort::new();
    labels.set_label(block.miner, Label::etherscan("Miner Label"));
    labels.set_label(
        Address::from_hex("0xabcdabcdabcdabcdabcdabcdabcdabcdabcdabcd").unwrap(),
        Label::etherscan("Unreachable"),
    );

    let view = load_block_overview::run_with_labels(
        &reader,
        &labels,
        BlockId::Number(block.number),
        Chain::Ethereum,
    )
    .await
    .expect("ok");

    assert_eq!(view.signer_label, None);
    // Only the miner was consulted.
    assert_eq!(labels.call_count(), 1);
}

#[tokio::test]
async fn label_port_error_does_not_break_overview_load() {
    let reader = StubBlockReaderPort::new();
    let block = sample_block(
        602,
        "0xd602000000000000000000000000000000000000000000000000000000000000",
    );
    reader.insert(block.clone());

    let labels = StubLabelPort::new();
    labels.fail_with(DomainError::ProviderUnavailable);

    let view = load_block_overview::run_with_labels(
        &reader,
        &labels,
        BlockId::Number(block.number),
        Chain::Ethereum,
    )
    .await
    .expect("label error must not propagate");

    assert_eq!(view.block, block);
    assert_eq!(view.miner_label, None);
}

#[tokio::test]
async fn missing_block_still_errors_through_run_with_labels() {
    let reader = StubBlockReaderPort::new();
    let labels = StubLabelPort::new();

    let err = load_block_overview::run_with_labels(
        &reader,
        &labels,
        BlockId::Number(BlockNumber::new(404)),
        Chain::Ethereum,
    )
    .await
    .expect_err("missing block must error");

    assert!(matches!(err, DomainError::NotFound));
}
