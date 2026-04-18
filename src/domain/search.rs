//! Types produced by the universal-search use case.
//!
//! See `plan/2-search.md` section 4.

use crate::domain::{Address, BlockHash, BlockNumber, TokenMetadata, TxHash};

/// EOA vs contract. Returned by `AddressLookupPort::classify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressKind {
    Eoa,
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
            ResolvedEntity::Token(_) => "token",
            ResolvedEntity::NotFound { .. } => "not found",
        }
    }
}
