//! Functional tests for the `load_address_transfers` use case.
//!
//! See `plan/6-address-detail.md` section 12.4.1.

use blockexplorer_tui::{
    application::use_cases::load_address_transfers,
    domain::{
        Address, BlockNumber, Chain, TransferAsset, TransferCategory, TransferEvent,
        TransferPage, TxHash, Wei,
    },
};
use pretty_assertions::assert_eq;

use crate::support::stubs::StubTransfersPort;

fn sample_event(block: u64, hash_hex: &str) -> TransferEvent {
    TransferEvent {
        chain: Chain::Ethereum,
        block_number: BlockNumber::new(block),
        tx_hash: TxHash::from_hex(hash_hex).unwrap(),
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()),
        asset: TransferAsset::Native {
            symbol: "ETH".into(),
        },
        value: Wei::new(1_000_000_000_000_000_000),
        category: TransferCategory::External,
    }
}

#[tokio::test]
async fn returns_the_primed_page() {
    let transfers = StubTransfersPort::new();
    let address = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    let events = vec![
        sample_event(
            21_000_000,
            "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
        ),
        sample_event(
            20_999_999,
            "0xfefe016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944f",
        ),
    ];
    transfers.set_page(
        address,
        TransferPage {
            events: events.clone(),
            next_cursor: None,
        },
    );

    let page = load_address_transfers::run(&transfers, address, Chain::Ethereum, None)
        .await
        .expect("ok");
    assert_eq!(page.events, events);
    assert!(page.next_cursor.is_none());
}

#[tokio::test]
async fn missing_address_returns_empty_page() {
    let transfers = StubTransfersPort::new();
    let address = Address::from_hex("0x0000000000000000000000000000000000000099").unwrap();

    let page = load_address_transfers::run(&transfers, address, Chain::Ethereum, None)
        .await
        .expect("ok");
    assert!(page.events.is_empty());
    assert!(page.next_cursor.is_none());
}
