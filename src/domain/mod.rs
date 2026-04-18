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
pub mod contract;
pub mod contract_read;
pub mod contract_source;
pub mod errors;
pub mod gas;
pub mod mempool;
pub mod network_status;
pub mod portfolio;
pub mod search;
pub mod timestamp;
pub mod token;
pub mod transfers;
pub mod tx;
pub mod tx_simulation;
pub mod tx_trace;

pub use address::{Address, AddressOverview};
pub use block::{Block, BlockHash, BlockId, BlockNumber, BlockSummary};
pub use chain::Chain;
pub use contract::{ContractOverview, ProxyInfo, ProxyKind};
pub use contract_read::{
    AbiFunction, AbiParam, AbiParamType, AbiValue, DecodedValue, parse_abi_functions,
};
pub use contract_source::{ContractAbi, ContractSource, SourceFile};
pub use errors::DomainError;
pub use gas::{Gwei, Wei};
pub use mempool::{PendingTx, PendingTxEvent, PendingTxFilter};
pub use network_status::{GasSnapshot, NetworkStatus};
pub use portfolio::TokenHolding;
pub use search::{AddressKind, ResolvedEntity};
pub use timestamp::UnixTimestamp;
pub use token::{TokenMetadata, TokenOverview};
pub use transfers::{
    NftKind, TransferAsset, TransferCategory, TransferCursor, TransferEvent, TransferPage,
};
pub use tx::{LogEntry, Transaction, TxHash, TxStatus, TxSummary, TxType};
pub use tx_simulation::{AssetChange, AssetChangeKind, AssetKind};
pub use tx_trace::{AddressStateDiff, DiffChange, StateDiff, StorageSlotDiff};
