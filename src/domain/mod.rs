//! Domain layer: pure types and invariants.
//!
//! Nothing in this module performs I/O. Value objects and entities live in
//! their own files and expose fallible constructors returning
//! [`DomainError`].
//!
//! See `plan/0-general-architecture.md` section 6 for layer rules.

pub mod address;
pub mod block;
pub mod chain;
pub mod errors;
pub mod gas;
pub mod network_status;
pub mod search;
pub mod timestamp;
pub mod token;
pub mod tx;

pub use address::Address;
pub use block::{Block, BlockHash, BlockId, BlockNumber, BlockSummary};
pub use chain::Chain;
pub use errors::DomainError;
pub use gas::{Gwei, Wei};
pub use network_status::{GasSnapshot, NetworkStatus};
pub use search::{AddressKind, ResolvedEntity};
pub use timestamp::UnixTimestamp;
pub use token::TokenMetadata;
pub use tx::{Transaction, TxHash, TxStatus, TxSummary, TxType};
