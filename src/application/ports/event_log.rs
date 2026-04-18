//! Outbound port returning log entries for a contract over a block
//! range. Backed by `eth_getLogs` in live mode.
//!
//! See `plan/7-contract-detail.md` section 12.4.3.

use crate::domain::{Address, BlockNumber, Chain, DomainError, LogEntry};

/// Inclusive block-range window for [`EventLogPort::get_logs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockRange {
    pub from: BlockNumber,
    pub to: BlockNumber,
}

pub trait EventLogPort: Send + Sync {
    fn get_logs(
        &self,
        address: Address,
        chain: Chain,
        range: BlockRange,
    ) -> impl std::future::Future<Output = Result<Vec<LogEntry>, DomainError>> + Send;
}
