//! Functional tests for `load_address_account_transactions`.
//!
//! See `plan/18-shell-navigation-and-feeds.md` Slice D.

use blockexplorer_tui::{
    application::use_cases::load_address_account_transactions,
    domain::{AccountTx, AccountTxPage, Address, BlockNumber, Chain, TxHash, Wei},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::StubAccountTransactionsPort;

fn sample_tx(block: u64, hash_hex: &str) -> AccountTx {
    AccountTx {
        chain: Chain::Ethereum,
        block_number: BlockNumber::new(block),
        tx_hash: TxHash::from_hex(hash_hex).unwrap(),
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()),
        value: Wei::new(1),
    }
}

#[tokio::test]
async fn returns_the_primed_page() {
    let port = StubAccountTransactionsPort::new();
    let address = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    let txs = vec![
        sample_tx(
            21_000_000,
            "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
        ),
        sample_tx(
            20_999_999,
            "0xfefe016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944f",
        ),
    ];
    port.set_page(
        address,
        AccountTxPage {
            txs: txs.clone(),
            next_cursor: None,
        },
    );

    let page = load_address_account_transactions::run(&port, address, Chain::Ethereum, None)
        .await
        .expect("ok");
    assert_eq!(page.txs, txs);
    assert!(page.next_cursor.is_none());
}

#[tokio::test]
async fn missing_address_returns_empty_page() {
    let port = StubAccountTransactionsPort::new();
    let address = Address::from_hex("0x0000000000000000000000000000000000000099").unwrap();

    let page = load_address_account_transactions::run(&port, address, Chain::Ethereum, None)
        .await
        .expect("ok");
    assert!(page.txs.is_empty());
    assert!(page.next_cursor.is_none());
}
