//! Alchemy Prices API adapter.
//!
//! The Prices API lives on a different host than the JSON-RPC
//! endpoint (`https://api.g.alchemy.com/prices/v1/...`), so it owns a
//! dedicated REST client rather than reusing the `RpcClient`.
//!
//! See `plan/8-token-detail.md` section 12.4 and the Alchemy docs at
//! <https://www.alchemy.com/docs/data>.

pub mod alchemy_prices;
pub mod client;

pub use alchemy_prices::AlchemyPrices;
pub use client::{PricesClient, PricesError};
