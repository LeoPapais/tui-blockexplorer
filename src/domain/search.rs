//! Types produced by the universal-search use case.
//!
//! See `plan/2-search.md` section 4.

use crate::domain::{Address, BlockHash, BlockNumber, TokenMetadata, TxHash};

/// EOA vs contract, with optional EIP-7702 delegation designator.
///
/// Returned by `AddressLookupPort::classify`. `Eoa { delegated_to:
/// Some(_) }` means the account is a plain wallet whose code slot
/// carries the 23-byte EIP-7702 delegation tag (`0xef0100` + 20-byte
/// delegate). See `plan/15-backlog.md` section 3.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressKind {
    Eoa { delegated_to: Option<Address> },
    Contract,
}

/// One candidate produced by `ResolveQuery`. Ordered by relevance in
/// the returned `Vec`; the first element is the one the UI should
/// highlight by default.
///
/// `Contract { address }` is emitted alongside `Address { kind:
/// Contract, .. }` when a contract address is searched, so the user
/// can jump straight to the ContractDetail screen. `Token(_)` is
/// emitted by the search feed (not by `ResolveQuery`) when a
/// follow-up ERC-20 probe succeeds; see `plan/2-search.md` section
/// 11.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedEntity {
    Block {
        number: BlockNumber,
        hash: BlockHash,
    },
    Tx {
        hash: TxHash,
        block: Option<BlockNumber>,
    },
    Address {
        address: Address,
        kind: AddressKind,
        ens_name: Option<String>,
    },
    /// Shortcut entry that opens the ContractDetail screen directly.
    /// Emitted whenever the underlying address resolves to a
    /// contract, independently of whether the contract is an ERC-20.
    Contract {
        address: Address,
    },
    /// An EOA that has opted into an EIP-7702 delegation designator.
    /// Emitted **instead of** a plain `Address` row so the Search
    /// list can render the wallet with a delegation hint next to it.
    /// No `Contract` shortcut is produced for this case: the account
    /// is not a contract. See `plan/15-backlog.md` section 3.1.
    DelegatedEoa {
        address: Address,
        delegated_to: Address,
    },
    Token(TokenMetadata),
    NotFound {
        reason: String,
    },
}

impl ResolvedEntity {
    /// Short label used on the search result list.
    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        match self {
            ResolvedEntity::Block { .. } => "block",
            ResolvedEntity::Tx { .. } => "transaction",
            ResolvedEntity::Address { .. } => "address",
            ResolvedEntity::Contract { .. } => "contract",
            ResolvedEntity::DelegatedEoa { .. } => "address",
            ResolvedEntity::Token(_) => "token",
            ResolvedEntity::NotFound { .. } => "not found",
        }
    }
}
