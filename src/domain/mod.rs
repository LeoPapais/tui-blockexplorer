//! Domain layer: pure types and invariants.
//!
//! Nothing in this module performs I/O. Value objects and entities live in
//! their own files and expose fallible constructors returning
//! [`DomainError`].
//!
//! See `plan/0-general-architecture.md` section 6 for layer rules.

pub mod block;
pub mod chain;
pub mod errors;
pub mod gas;
pub mod network_status;

pub use block::BlockNumber;
pub use chain::Chain;
pub use errors::DomainError;
pub use gas::{Gwei, Wei};
pub use network_status::{GasSnapshot, NetworkStatus};
