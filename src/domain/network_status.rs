//! Read models for the Home screen: `NetworkStatus` and `GasSnapshot`.
//!
//! These are DTO-style value types rather than full entities; they exist to
//! carry port output to the Application layer. See `plan/1-home.md`
//! section 11.1.

use crate::domain::{BlockNumber, Chain, Gwei, Wei};

/// Snapshot of the chain's head plus derived statistics used on the
/// Network card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkStatus {
    pub chain: Chain,
    pub latest_block: BlockNumber,
    pub base_fee: Wei,
    /// Rolling average block time in milliseconds computed over the last
    /// N blocks. See `plan/1-home.md` section 4.1.
    pub block_time_avg_ms: u64,
}

/// Minimal event emitted by the `newHeads` WebSocket subscription and
/// consumed by the Home dispatcher. See `plan/1-home.md` section 12.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewHead {
    pub chain: Chain,
    pub number: BlockNumber,
}

/// Slow / average / fast gwei tiers plus extra context used on the Gas
/// Tracker card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GasSnapshot {
    pub chain: Chain,
    pub slow: Gwei,
    pub average: Gwei,
    pub fast: Gwei,
    pub base_fee: Gwei,
    /// Most recent base-fee samples, newest last. Used to draw the trend
    /// sparkline.
    pub trend: Vec<Gwei>,
}
