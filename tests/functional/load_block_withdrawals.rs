//! Functional tests for `load_block_withdrawals`.
//!
//! See `plan/3-block-detail.md` §12.5.

use blockexplorer_tui::{
    application::use_cases::load_block_withdrawals,
    domain::{Address, Block, BlockHash, BlockNumber, Chain, UnixTimestamp, Wei, Withdrawal},
};
use pretty_assertions::assert_eq;

fn block_with_withdrawals(withdrawals: Vec<Withdrawal>) -> Block {
    Block {
        chain: Chain::Ethereum,
        number: BlockNumber::new(21_345_679),
        hash: BlockHash::from_hex(
            "0xaaaa000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap(),
        parent_hash: BlockHash::from_hex(
            "0xbbbb000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap(),
        timestamp: UnixTimestamp::from_seconds(1_710_000_001),
        miner: Address::from_hex("0x1111111111111111111111111111111111111111").unwrap(),
        gas_used: 12_000_000,
        gas_limit: 30_000_000,
        base_fee: Some(Wei::new(11_400_000_000)),
        size: 102_400,
        extra_data: vec![0x42, 0x42],
        tx_hashes: Vec::new(),
        extra_signer: None,
        withdrawals,
    }
}

#[test]
fn returns_the_parsed_withdrawals_verbatim() {
    let sample = vec![
        Withdrawal {
            index: 2_000_000,
            validator_index: 48_879,
            address: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
            amount_gwei: 1_000_000_000,
        },
        Withdrawal {
            index: 2_000_001,
            validator_index: 61_453,
            address: Address::from_hex("0x5abc0e99dfc7ba2c9da42f8dc91ec4128a89e919").unwrap(),
            amount_gwei: 500_000_000,
        },
    ];
    let block = block_with_withdrawals(sample.clone());

    let got = load_block_withdrawals::run(&block);

    assert_eq!(got, sample);
}

#[test]
fn empty_block_returns_an_empty_vec() {
    let block = block_with_withdrawals(Vec::new());

    let got = load_block_withdrawals::run(&block);

    assert!(got.is_empty());
}
