//! Functional tests for `load_token_transfers`.
//!
//! See `plan/8-token-detail.md` section 4.2.

use blockexplorer_tui::{
    application::use_cases::load_token_transfers,
    domain::{
        Address, BlockNumber, Chain, TransferAsset, TransferCategory, TransferEvent, TransferPage,
        TxHash, Wei,
    },
};
use pretty_assertions::assert_eq;

use crate::support::stubs::StubTransfersPort;

fn usdc() -> Address {
    Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
}

fn alice() -> Address {
    Address::from_hex("0x1111111111111111111111111111111111111111").unwrap()
}

fn bob() -> Address {
    Address::from_hex("0x2222222222222222222222222222222222222222").unwrap()
}

fn sample_page() -> TransferPage {
    let hash =
        TxHash::from_hex("0xdeadbeef00000000000000000000000000000000000000000000000000000001")
            .unwrap();
    TransferPage {
        events: vec![TransferEvent {
            chain: Chain::Ethereum,
            block_number: BlockNumber::new(21_345_678),
            tx_hash: hash,
            from: alice(),
            to: Some(bob()),
            asset: TransferAsset::Erc20 {
                contract: usdc(),
                symbol: "USDC".into(),
                decimals: 6,
            },
            value: Wei::new(1_000_000),
            category: TransferCategory::Erc20,
        }],
        next_cursor: None,
    }
}

#[tokio::test]
async fn happy_path_returns_primed_page() {
    let transfers = StubTransfersPort::new();
    transfers.set_page_for_contract(usdc(), sample_page());

    let page = load_token_transfers::run(&transfers, usdc(), Chain::Ethereum, None)
        .await
        .expect("ok");

    assert_eq!(page.events.len(), 1);
    assert_eq!(page.events[0].from, alice());
    assert_eq!(page.events[0].to, Some(bob()));
}

#[tokio::test]
async fn missing_contract_returns_empty_page() {
    let transfers = StubTransfersPort::new();

    let page = load_token_transfers::run(&transfers, usdc(), Chain::Ethereum, None)
        .await
        .expect("empty is fine");

    assert!(page.events.is_empty());
    assert!(page.next_cursor.is_none());
}
