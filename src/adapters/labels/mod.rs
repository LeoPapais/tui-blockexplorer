//! Adapters that implement [`LabelPort`](crate::application::ports::LabelPort).
//!
//! Two concrete strategies plus a composite:
//!
//! - `WellKnownLabels` — static, in-binary dictionary of common
//!   exchange wallets, validator signers, bridges, ... Good for
//!   offline scenarios and as a deterministic fallback.
//! - `EtherscanLabel` lives under
//!   `crate::adapters::etherscan::label` since it shares the
//!   Etherscan V2 HTTP client with the other modules there.
//! - `CompositeLabels` — tries well-known first, falls back to the
//!   live adapter. Mirrors `CompositeSignatureDirectory`.
//!
//! See `plan/3-block-detail.md` §12.4.

pub mod composite;
pub mod well_known;

pub use composite::CompositeLabels;
pub use well_known::WellKnownLabels;
