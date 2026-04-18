//! Use case: load one windowed page of event logs for the Contract
//! Detail Events tab.
//!
//! Composes [`NetworkStatusPort`] (resolve the chain head once) with
//! [`EventLogPort`] (read a 5 000-block window). Successive pages
//! reuse the cached head so windows stay aligned as the user pages
//! backwards.
//!
//! See `plan/7-contract-detail.md` section 12.5.3.

use crate::{
    application::ports::{BlockRange, EventLogPort, NetworkStatusPort},
    domain::{Address, BlockNumber, Chain, DomainError, EventsPage},
};

/// Default pagination window size. Picked so `eth_getLogs` stays
/// under provider payload limits.
pub const DEFAULT_WINDOW_BLOCKS: u64 = 5_000;

/// Load page `offset` (0 = newest). When `head_hint` is `None` the
/// chain head is resolved through [`NetworkStatusPort::snapshot`];
/// callers that already hold a fresh head pass it in to avoid the
/// extra round-trip.
pub async fn run<N, E>(
    network_status: &N,
    event_log: &E,
    address: Address,
    chain: Chain,
    head_hint: Option<BlockNumber>,
    offset: u32,
) -> Result<EventsPage, DomainError>
where
    N: NetworkStatusPort,
    E: EventLogPort,
{
    let head = match head_hint {
        Some(h) => h,
        None => network_status.snapshot(chain).await?.latest_block,
    };
    let head_val = head.value();
    let off = u64::from(offset);
    let jump = off.saturating_mul(DEFAULT_WINDOW_BLOCKS);

    // Paging beyond the chain start: surface an empty page so the
    // UI can render "no events in this window" instead of crashing.
    if jump > head_val {
        return Ok(EventsPage {
            head,
            window_from: BlockNumber::new(0),
            window_to: BlockNumber::new(0),
            logs: Vec::new(),
            has_older: false,
        });
    }

    let top = head_val - jump;
    let bottom = top.saturating_sub(DEFAULT_WINDOW_BLOCKS.saturating_sub(1));
    let window = BlockRange {
        from: BlockNumber::new(bottom),
        to: BlockNumber::new(top),
    };
    let logs = event_log.get_logs(address, chain, window).await?;
    let has_older = bottom > 0;

    Ok(EventsPage {
        head,
        window_from: BlockNumber::new(bottom),
        window_to: BlockNumber::new(top),
        logs,
        has_older,
    })
}
