//! Universal-search use case.
//!
//! Classifies the user input, fires every plausible lookup in parallel
//! and ranks the results. See `plan/2-search.md` sections 3 and 4.

use crate::{
    application::ports::{
        AddressLookupPort, BlockLookupPort, EnsResolverPort, TokenSearchPort, TxLookupPort,
    },
    domain::{
        Address, BlockHash, BlockNumber, Chain, DomainError, ResolvedEntity, TxHash,
    },
};

/// Five-port coordinator that produces a ranked list of candidates for
/// the search input.
pub struct ResolveQuery<'a, B, T, A, E, S>
where
    B: BlockLookupPort,
    T: TxLookupPort,
    A: AddressLookupPort,
    E: EnsResolverPort,
    S: TokenSearchPort,
{
    pub block: &'a B,
    pub tx: &'a T,
    pub address: &'a A,
    pub ens: &'a E,
    pub token: &'a S,
}

impl<B, T, A, E, S> ResolveQuery<'_, B, T, A, E, S>
where
    B: BlockLookupPort,
    T: TxLookupPort,
    A: AddressLookupPort,
    E: EnsResolverPort,
    S: TokenSearchPort,
{
    /// Execute the resolver. Returns at least one entry: a
    /// [`ResolvedEntity::NotFound`] when every plausible lookup came
    /// back empty.
    pub async fn run(
        &self,
        input: &str,
        chain: Chain,
    ) -> Result<Vec<ResolvedEntity>, DomainError> {
        let normalized = input.trim();
        if normalized.is_empty() {
            return Err(DomainError::InvalidInput("empty search input".into()));
        }

        let classification = classify(normalized);
        let mut candidates = Vec::new();

        match classification {
            Classification::Hash32 { lower_hex } => {
                let tx_hash = TxHash::from_hex(&lower_hex)?;
                let block_hash = BlockHash::from_hex(&lower_hex)?;
                let (tx_r, block_r) = tokio::join!(
                    self.tx.get(tx_hash, chain),
                    self.block.get_by_hash(block_hash, chain),
                );
                if let Some(tx) = tx_r? {
                    candidates.push(ResolvedEntity::Tx {
                        hash: tx.hash,
                        block: tx.block,
                    });
                }
                if let Some(b) = block_r? {
                    candidates.push(ResolvedEntity::Block {
                        number: b.number,
                        hash: b.hash,
                    });
                }
            }
            Classification::Address { lower_hex } => {
                let addr = Address::from_hex(&lower_hex)?;
                let (kind_r, rev_r) = tokio::join!(
                    self.address.classify(addr, chain),
                    self.ens.reverse(addr, chain),
                );
                let kind = kind_r?;
                let ens_name = rev_r.ok().flatten();
                candidates.push(ResolvedEntity::Address {
                    address: addr,
                    kind,
                    ens_name,
                });
            }
            Classification::BlockNumber(n) => {
                if let Some(b) = self.block.get_by_number(n, chain).await? {
                    candidates.push(ResolvedEntity::Block {
                        number: b.number,
                        hash: b.hash,
                    });
                }
            }
            Classification::EnsName(name) => {
                let forward = self.ens.forward(&name, chain).await?;
                if let Some(addr) = forward {
                    let kind = self
                        .address
                        .classify(addr, chain)
                        .await
                        .unwrap_or(crate::domain::AddressKind::Eoa);
                    candidates.push(ResolvedEntity::Address {
                        address: addr,
                        kind,
                        ens_name: Some(name),
                    });
                }
            }
            Classification::TokenTicker(symbol) => {
                let matches = self.token.by_symbol(&symbol, chain).await?;
                for meta in matches {
                    candidates.push(ResolvedEntity::Token(meta));
                }
            }
            Classification::FreeText(text) => {
                let matches = self.token.by_name(&text, chain).await?;
                for meta in matches {
                    candidates.push(ResolvedEntity::Token(meta));
                }
            }
        }

        if candidates.is_empty() {
            candidates.push(ResolvedEntity::NotFound {
                reason: format!(
                    "no match for \"{}\" on {}",
                    normalized,
                    chain.display_name()
                ),
            });
        }

        Ok(candidates)
    }
}

/// Classification of the normalised input. Exposed for unit testing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classification {
    Hash32 { lower_hex: String },
    Address { lower_hex: String },
    BlockNumber(BlockNumber),
    EnsName(String),
    TokenTicker(String),
    FreeText(String),
}

/// Decide which lookups to run for `input`. `input` must already be
/// trimmed.
#[must_use]
pub fn classify(input: &str) -> Classification {
    let lower = input.to_lowercase();

    if is_prefixed_hex(&lower, 64) {
        return Classification::Hash32 { lower_hex: lower };
    }
    if is_prefixed_hex(&lower, 40) {
        return Classification::Address { lower_hex: lower };
    }
    if input.chars().all(|c| c.is_ascii_digit())
        && let Ok(n) = input.parse::<u64>()
    {
        return Classification::BlockNumber(BlockNumber::new(n));
    }
    if lower.ends_with(".eth") {
        return Classification::EnsName(lower);
    }
    if is_ticker(input) {
        return Classification::TokenTicker(input.to_uppercase());
    }
    Classification::FreeText(input.to_string())
}

fn is_prefixed_hex(lower: &str, body_len: usize) -> bool {
    let Some(body) = lower.strip_prefix("0x") else {
        return false;
    };
    body.len() == body_len && body.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_ticker(input: &str) -> bool {
    let len = input.chars().count();
    (2..=10).contains(&len) && input.chars().all(|c| c.is_ascii_uppercase())
}
