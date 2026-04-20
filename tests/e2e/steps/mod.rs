//! Step definitions grouped per screen.
//!
//! Every `{screen}.rs` file registers its `#[given] / #[when] / #[then]`
//! functions against the shared [`crate::world::AppWorld`]. The step
//! functions stay mostly unimplemented during the scaffolding phase so
//! scenarios fail loudly with a reference to the plan file they come from.

pub mod address_detail;
pub mod block_detail;
pub mod gas_tracker;
pub mod home;
pub mod mempool;
pub mod search;
pub mod settings;
pub mod shared;
pub mod tx_detail;
