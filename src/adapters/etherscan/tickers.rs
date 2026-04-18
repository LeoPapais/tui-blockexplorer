//! Curated, hand-maintained ERC-20 ticker table used by
//! [`super::token_search::EtherscanTokenSearch`].
//!
//! The table intentionally stays small: the MVP only needs the
//! blue-chip tokens per chain that users actually search for by
//! symbol. Adding / editing entries is a code change so the table
//! never ships secrets or live market data.
//!
//! See `plan/2-search.md` section 10.2.

use crate::domain::Chain;

/// Per-chain ERC-20 snapshot: `(ticker_uppercase, name, address_lowercase_hex, decimals)`.
///
/// The `address_hex` field is the canonical 42-character lowercase hex
/// representation (`0x` prefix + 40 hex chars). Keeping the parsing
/// deferred to the adapter ensures a malformed entry fails loudly
/// during the one-and-only-time it is read.
#[derive(Debug, Clone, Copy)]
pub struct TickerEntry {
    pub ticker: &'static str,
    pub name: &'static str,
    pub address_hex: &'static str,
    pub decimals: u8,
}

/// Return every curated entry for `chain`.
#[must_use]
pub const fn tickers_for(chain: Chain) -> &'static [TickerEntry] {
    match chain {
        Chain::Ethereum => ETHEREUM,
        Chain::EthereumSepolia => SEPOLIA,
        Chain::Base => BASE,
        Chain::Polygon => POLYGON,
        Chain::Optimism => OPTIMISM,
        Chain::Arbitrum => ARBITRUM,
    }
}

const ETHEREUM: &[TickerEntry] = &[
    TickerEntry {
        ticker: "USDC",
        name: "USD Coin",
        address_hex: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        decimals: 6,
    },
    TickerEntry {
        ticker: "USDT",
        name: "Tether USD",
        address_hex: "0xdac17f958d2ee523a2206206994597c13d831ec7",
        decimals: 6,
    },
    TickerEntry {
        ticker: "DAI",
        name: "Dai Stablecoin",
        address_hex: "0x6b175474e89094c44da98b954eedeac495271d0f",
        decimals: 18,
    },
    TickerEntry {
        ticker: "WETH",
        name: "Wrapped Ether",
        address_hex: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
        decimals: 18,
    },
    TickerEntry {
        ticker: "WBTC",
        name: "Wrapped BTC",
        address_hex: "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599",
        decimals: 8,
    },
];

const SEPOLIA: &[TickerEntry] = &[];

const BASE: &[TickerEntry] = &[
    TickerEntry {
        ticker: "USDC",
        name: "USD Coin",
        address_hex: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",
        decimals: 6,
    },
    TickerEntry {
        ticker: "WETH",
        name: "Wrapped Ether",
        address_hex: "0x4200000000000000000000000000000000000006",
        decimals: 18,
    },
];

const POLYGON: &[TickerEntry] = &[
    TickerEntry {
        ticker: "USDC",
        name: "USD Coin",
        address_hex: "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359",
        decimals: 6,
    },
    TickerEntry {
        ticker: "USDT",
        name: "Tether USD",
        address_hex: "0xc2132d05d31c914a87c6611c10748aeb04b58e8f",
        decimals: 6,
    },
    TickerEntry {
        ticker: "DAI",
        name: "Dai Stablecoin",
        address_hex: "0x8f3cf7ad23cd3cadbd9735aff958023239c6a063",
        decimals: 18,
    },
    TickerEntry {
        ticker: "WETH",
        name: "Wrapped Ether",
        address_hex: "0x7ceb23fd6bc0add59e62ac25578270cff1b9f619",
        decimals: 18,
    },
];

const OPTIMISM: &[TickerEntry] = &[
    TickerEntry {
        ticker: "USDC",
        name: "USD Coin",
        address_hex: "0x0b2c639c533813f4aa9d7837caf62653d097ff85",
        decimals: 6,
    },
    TickerEntry {
        ticker: "WETH",
        name: "Wrapped Ether",
        address_hex: "0x4200000000000000000000000000000000000006",
        decimals: 18,
    },
];

const ARBITRUM: &[TickerEntry] = &[
    TickerEntry {
        ticker: "USDC",
        name: "USD Coin",
        address_hex: "0xaf88d065e77c8cc2239327c5edb3a432268e5831",
        decimals: 6,
    },
    TickerEntry {
        ticker: "WETH",
        name: "Wrapped Ether",
        address_hex: "0x82af49447d8a07e3bd95bd0d56f35241523fbab1",
        decimals: 18,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Address;

    #[test]
    fn every_entry_parses_as_address() {
        for chain in Chain::all() {
            for entry in tickers_for(*chain) {
                Address::from_hex(entry.address_hex).unwrap_or_else(|e| {
                    panic!(
                        "malformed ticker entry on {:?} for {}: {e}",
                        chain, entry.ticker
                    )
                });
            }
        }
    }

    #[test]
    fn no_duplicate_tickers_per_chain() {
        for chain in Chain::all() {
            let entries = tickers_for(*chain);
            for i in 0..entries.len() {
                for j in (i + 1)..entries.len() {
                    assert_ne!(
                        entries[i].ticker, entries[j].ticker,
                        "{:?} has duplicate ticker {}",
                        chain, entries[i].ticker,
                    );
                }
            }
        }
    }
}
