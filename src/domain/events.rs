//! Page-shaped view-model for the Contract Detail Events tab.
//!
//! See `plan/7-contract-detail.md` section 12.5.3.

use crate::domain::{BlockNumber, LogEntry};

/// One windowed page of event logs. The UI keeps the
/// `(offset, head)` pair to drive pagination and surfaces
/// `window_from..=window_to` in the tab header.
///
/// `window_from` / `window_to` intentionally mirror `BlockRange`'s
/// shape without the cross-layer import: the domain stays a
/// pure-types module even when the adapter layer owns the HTTP-level
/// filter type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventsPage {
    /// The chain head resolved when the page was built. Reused
    /// across subsequent page fetches so windows stay aligned as
    /// the user walks backwards.
    pub head: BlockNumber,
    /// Inclusive start of the queried window.
    pub window_from: BlockNumber,
    /// Inclusive end of the queried window.
    pub window_to: BlockNumber,
    /// Logs returned by the adapter for this window.
    pub logs: Vec<LogEntry>,
    /// `true` when older blocks exist below `window_from` (the
    /// "next window" action is enabled).
    pub has_older: bool,
}
