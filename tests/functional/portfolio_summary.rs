//! Functional tests for the Portfolio summary helper that backs
//! the Tokens tab header + top-5 distribution chart.
//!
//! See `plan/6-address-detail.md` §11 "Shipped" (promoted from
//! `plan/15-backlog.md` §8.7). The aggregator is a pure function
//! so the tests stay independent of any terminal wiring.

use blockexplorer_tui::{
    adapters::ui::address_detail::portfolio_summary,
    domain::{Address, PriceLookup, TokenHolding, TokenMetadata, TokenPrice, UnixTimestamp, Wei},
};
use pretty_assertions::assert_eq;

fn addr(hex: &str) -> Address {
    Address::from_hex(hex).unwrap()
}

fn priced(symbol: &str, contract: &str, balance: u128, decimals: u8, usd: f64) -> TokenHolding {
    TokenHolding {
        metadata: TokenMetadata {
            address: addr(contract),
            symbol: symbol.into(),
            name: format!("{symbol} Token"),
            decimals,
        },
        balance: Wei::new(balance),
        price: PriceLookup::Available(TokenPrice {
            currency: "usd".into(),
            value: usd,
            as_of: UnixTimestamp::from_seconds(1),
        }),
    }
}

fn unsupported(symbol: &str, contract: &str, balance: u128, decimals: u8) -> TokenHolding {
    TokenHolding {
        metadata: TokenMetadata {
            address: addr(contract),
            symbol: symbol.into(),
            name: format!("{symbol} Token"),
            decimals,
        },
        balance: Wei::new(balance),
        price: PriceLookup::Unsupported {
            provider: "alchemy-prices",
        },
    }
}

fn pending(symbol: &str, contract: &str, balance: u128, decimals: u8) -> TokenHolding {
    TokenHolding {
        metadata: TokenMetadata {
            address: addr(contract),
            symbol: symbol.into(),
            name: format!("{symbol} Token"),
            decimals,
        },
        balance: Wei::new(balance),
        price: PriceLookup::Pending,
    }
}

#[test]
fn sums_usd_across_priced_holdings_only() {
    let holdings = vec![
        // 1_000_000 * 10^-6 * 1.0001 = 1.0001 USD
        priced(
            "USDC",
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
            1_000_000,
            6,
            1.0001,
        ),
        // 5_000_000_000_000_000_000 * 10^-18 * 0.2 = 1.0 USD
        priced(
            "BRLA",
            "0xe6a537a407488807f0bbeb0038b79004f19dddfb",
            5_000_000_000_000_000_000,
            18,
            0.2,
        ),
        unsupported("WEIRD", "0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead", 1, 18),
        pending("PENDY", "0xbabebabebabebabebabebabebabebabebabebabe", 2, 18),
    ];
    let summary = portfolio_summary(&holdings);
    assert!(
        (summary.total_usd - 2.0001).abs() < 1e-6,
        "total USD differs: got {}",
        summary.total_usd,
    );
    assert_eq!(summary.not_priced, 2);
    assert_eq!(summary.priced, 2);
}

#[test]
fn top_five_is_ordered_by_usd_value_desc_and_capped_at_five() {
    let mut holdings = Vec::new();
    // Seven priced holdings with USD values 1..=7.
    for i in 1..=7 {
        holdings.push(priced(
            &format!("T{i}"),
            &format!("0x{:040x}", 0x1_0000_0000_u64 + i as u64),
            1_000_000_000_000_000_000, // 1 token
            18,
            i as f64,
        ));
    }
    let summary = portfolio_summary(&holdings);
    let symbols: Vec<&str> = summary
        .top_by_usd
        .iter()
        .map(|t| t.symbol.as_str())
        .collect();
    assert_eq!(symbols, vec!["T7", "T6", "T5", "T4", "T3"]);
    for entry in &summary.top_by_usd {
        // Each entry of 1 token at i USD per token → USD value == price.
        assert!(entry.usd_value > 0.0);
    }
}

#[test]
fn empty_or_unpriced_holdings_yield_zero_total_and_empty_top() {
    let empty = portfolio_summary(&[]);
    assert_eq!(empty.total_usd, 0.0);
    assert_eq!(empty.priced, 0);
    assert_eq!(empty.not_priced, 0);
    assert!(empty.top_by_usd.is_empty());

    let only_unpriced = vec![
        unsupported("BRLA", "0xe6a537a407488807f0bbeb0038b79004f19dddfb", 1, 18),
        pending("PENDY", "0xbabebabebabebabebabebabebabebabebabebabe", 2, 18),
    ];
    let summary = portfolio_summary(&only_unpriced);
    assert_eq!(summary.total_usd, 0.0);
    assert_eq!(summary.priced, 0);
    assert_eq!(summary.not_priced, 2);
    assert!(summary.top_by_usd.is_empty());
}
