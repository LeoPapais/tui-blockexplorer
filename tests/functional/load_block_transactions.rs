//! Functional tests for `load_block_transactions`.
//!
//! Covers plan/3-block-detail.md §12.3: pagination, cancellation and
//! category plumbing through the `BlockReceiptsPort`.

use blockexplorer_tui::{
    application::{CancelFlag, use_cases::load_block_transactions},
    domain::{
        Address, BlockHash, BlockId, BlockNumber, BlockTxCursor, BlockTxReceipt, Chain,
        DomainError, TxCategory, TxHash, TxStatus, Wei,
    },
};
use pretty_assertions::assert_eq;

use crate::support::stubs::StubBlockReceiptsPort;

const BLOCK_NUMBER: u64 = 21_345_678;

fn tx_hash(seed: u8) -> TxHash {
    let mut bytes = [0u8; 32];
    bytes[0] = seed;
    TxHash::from_hex(&format!("0x{}", hex::encode(bytes))).unwrap()
}

fn addr(seed: u8) -> Address {
    let mut bytes = [0u8; 20];
    bytes[0] = seed;
    Address::from_bytes(bytes)
}

fn sample_rows() -> Vec<BlockTxReceipt> {
    vec![
        // index 0: plain ETH transfer
        BlockTxReceipt::from_fields(
            tx_hash(0x11),
            0,
            addr(0x01),
            Some(addr(0x02)),
            Wei::new(1_000_000_000_000_000_000),
            &[],
            Some(21_000),
            TxStatus::Success,
            None,
        ),
        // index 1: ERC20 transfer(address,uint256) call
        BlockTxReceipt::from_fields(
            tx_hash(0x22),
            1,
            addr(0x03),
            Some(addr(0x04)),
            Wei::new(0),
            &[0xa9, 0x05, 0x9c, 0xbb],
            Some(52_000),
            TxStatus::Success,
            None,
        ),
        // index 2: contract deployment
        BlockTxReceipt::from_fields(
            tx_hash(0x33),
            2,
            addr(0x05),
            None,
            Wei::new(0),
            &[0x60, 0x80],
            Some(1_234_567),
            TxStatus::Success,
            Some(addr(0x99)),
        ),
        // index 3: failed method call
        BlockTxReceipt::from_fields(
            tx_hash(0x44),
            3,
            addr(0x06),
            Some(addr(0x07)),
            Wei::new(0),
            &[0xde, 0xad, 0xbe, 0xef],
            Some(30_000),
            TxStatus::Failed {
                reason: Some("insufficient allowance".into()),
            },
            None,
        ),
    ]
}

fn build_stub() -> StubBlockReceiptsPort {
    let port = StubBlockReceiptsPort::new();
    port.set_by_number(BlockNumber::new(BLOCK_NUMBER), sample_rows());
    port
}

#[tokio::test]
async fn returns_the_first_page_when_offset_is_zero() {
    let port = build_stub();
    let cancel = CancelFlag::new();
    let cursor = BlockTxCursor {
        offset: 0,
        page_size: 2,
    };

    let page = load_block_transactions::run(
        &port,
        BlockId::Number(BlockNumber::new(BLOCK_NUMBER)),
        Chain::Ethereum,
        cursor,
        &cancel,
    )
    .await
    .expect("ok")
    .expect("not cancelled");

    assert_eq!(page.total, 4);
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.rows[0].tx_index, 0);
    assert_eq!(page.rows[1].tx_index, 1);
    assert_eq!(
        page.next,
        Some(BlockTxCursor {
            offset: 2,
            page_size: 2,
        })
    );
}

#[tokio::test]
async fn slices_the_next_page_using_the_cursor() {
    let port = build_stub();
    let cancel = CancelFlag::new();
    let cursor = BlockTxCursor {
        offset: 2,
        page_size: 2,
    };

    let page = load_block_transactions::run(
        &port,
        BlockId::Number(BlockNumber::new(BLOCK_NUMBER)),
        Chain::Ethereum,
        cursor,
        &cancel,
    )
    .await
    .expect("ok")
    .expect("not cancelled");

    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.rows[0].tx_index, 2);
    assert_eq!(page.rows[1].tx_index, 3);
    assert_eq!(page.next, None);
}

#[tokio::test]
async fn returns_empty_page_beyond_the_tail() {
    let port = build_stub();
    let cancel = CancelFlag::new();
    let cursor = BlockTxCursor {
        offset: 42,
        page_size: 5,
    };

    let page = load_block_transactions::run(
        &port,
        BlockId::Number(BlockNumber::new(BLOCK_NUMBER)),
        Chain::Ethereum,
        cursor,
        &cancel,
    )
    .await
    .expect("ok")
    .expect("not cancelled");

    assert_eq!(page.rows.len(), 0);
    assert_eq!(page.next, None);
    assert_eq!(page.total, 4);
}

#[tokio::test]
async fn categorises_rows_using_to_and_input() {
    let port = build_stub();
    let cancel = CancelFlag::new();
    let cursor = BlockTxCursor {
        offset: 0,
        page_size: 10,
    };

    let page = load_block_transactions::run(
        &port,
        BlockId::Number(BlockNumber::new(BLOCK_NUMBER)),
        Chain::Ethereum,
        cursor,
        &cancel,
    )
    .await
    .expect("ok")
    .expect("not cancelled");

    assert_eq!(page.rows[0].category, TxCategory::Transfer);
    assert_eq!(page.rows[1].category, TxCategory::Interaction);
    assert_eq!(page.rows[2].category, TxCategory::Deploy);
    assert_eq!(page.rows[3].category, TxCategory::Interaction);
}

#[tokio::test]
async fn maps_failed_receipts_to_failed_status() {
    let port = build_stub();
    let cancel = CancelFlag::new();

    let page = load_block_transactions::run(
        &port,
        BlockId::Number(BlockNumber::new(BLOCK_NUMBER)),
        Chain::Ethereum,
        BlockTxCursor {
            offset: 3,
            page_size: 1,
        },
        &cancel,
    )
    .await
    .unwrap()
    .unwrap();

    let failed = &page.rows[0];
    assert!(matches!(
        &failed.status,
        TxStatus::Failed {
            reason: Some(r),
        } if r == "insufficient allowance"
    ));
}

#[tokio::test]
async fn short_circuits_when_cancelled_before_the_call() {
    let port = build_stub();
    let cancel = CancelFlag::new();
    cancel.cancel();

    let page = load_block_transactions::run(
        &port,
        BlockId::Number(BlockNumber::new(BLOCK_NUMBER)),
        Chain::Ethereum,
        BlockTxCursor::first_page(),
        &cancel,
    )
    .await
    .expect("cancellation is not an error");

    assert_eq!(page, None, "cancelled runs must return Ok(None)");
    assert_eq!(
        port.call_count(),
        0,
        "port must not be reached after cancellation"
    );
}

#[tokio::test]
async fn propagates_domain_error_from_port() {
    let port = StubBlockReceiptsPort::new();
    port.fail_with(DomainError::ProviderUnavailable);
    let cancel = CancelFlag::new();

    let err = load_block_transactions::run(
        &port,
        BlockId::Number(BlockNumber::new(BLOCK_NUMBER)),
        Chain::Ethereum,
        BlockTxCursor::first_page(),
        &cancel,
    )
    .await
    .expect_err("provider failure must surface");

    assert!(matches!(err, DomainError::ProviderUnavailable));
}

#[tokio::test]
async fn lookup_by_hash_is_supported() {
    let hash =
        BlockHash::from_hex("0xaaaa000000000000000000000000000000000000000000000000000000000000")
            .unwrap();
    let port = StubBlockReceiptsPort::new();
    port.set_by_hash(hash, sample_rows());
    let cancel = CancelFlag::new();

    let page = load_block_transactions::run(
        &port,
        BlockId::Hash(hash),
        Chain::Ethereum,
        BlockTxCursor::first_page(),
        &cancel,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(page.rows.len(), 4);
}
