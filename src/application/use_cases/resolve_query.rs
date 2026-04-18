//! Universal-search use case.
//!
//! Classifies the user input, fires every plausible lookup in parallel
//! and ranks the results. See `plan/2-search.md` sections 3 and 4.

use crate::{
    application::ports::{
        AddressLookupPort, BlockLookupPort, EnsResolverPort, TokenSearchPort, TxLookupPort,
    },
    domain::{
        Address, AddressKind, BlockHash, BlockNumber, Chain, DomainError, ResolvedEntity, TxHash,
    },
};

/// Emit the Search rows for an address once its `AddressKind` is
/// known. Preserves the legacy "Address + optional Contract shortcut"
/// behaviour for contracts and plain EOAs, and emits a single
/// `DelegatedEoa` row when the address carries an EIP-7702 delegation
/// designator. See `plan/15-backlog.md` section 3.1.
fn push_address_candidates(
    candidates: &mut Vec<ResolvedEntity>,
    addr: Address,
    kind: AddressKind,
    ens_name: Option<String>,
) {
    match kind {
        AddressKind::Eoa {
            delegated_to: Some(delegate),
        } => {
            candidates.push(ResolvedEntity::DelegatedEoa {
                address: addr,
                delegated_to: delegate,
            });
        }
        AddressKind::Eoa { delegated_to: None } => {
            candidates.push(ResolvedEntity::Address {
                address: addr,
                kind,
                ens_name,
            });
        }
        AddressKind::Contract => {
            candidates.push(ResolvedEntity::Address {
                address: addr,
                kind,
                ens_name,
            });
            // Cosmetic shortcut: a second row that lands straight on
            // ContractDetail. No extra RPC: `classify` already did the
            // `eth_getCode`.
            candidates.push(ResolvedEntity::Contract { address: addr });
        }
    }
}

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
    pub async fn run(&self, input: &str, chain: Chain) -> Result<Vec<ResolvedEntity>, DomainError> {
        let ClassifiedInput {
            normalised,
            classification,
        } = classify_input(input);
        if normalised.is_empty() {
            return Err(DomainError::InvalidInput("empty search input".into()));
        }
        let normalized = normalised.as_str();
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
                push_address_candidates(&mut candidates, addr, kind, ens_name);
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
                        .unwrap_or(crate::domain::AddressKind::Eoa { delegated_to: None });
                    push_address_candidates(&mut candidates, addr, kind, Some(name));
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

/// Pair of the post-normalisation input and its classification.
///
/// Normalisation strips surrounding whitespace and one matching pair of
/// quotes, then unwraps block-explorer URLs so the trailing path
/// segment becomes the effective input. See `plan/2-search.md`
/// sections 12.1 and 12.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedInput {
    pub normalised: String,
    pub classification: Classification,
}

/// Hosts we recognise as block-explorer URLs. Keep this list in sync
/// with `plan/2-search.md` section 12.2.
const EXPLORER_HOSTS: &[&str] = &[
    "etherscan.io",
    "www.etherscan.io",
    "sepolia.etherscan.io",
    "optimistic.etherscan.io",
    "polygonscan.com",
    "www.polygonscan.com",
    "basescan.org",
    "www.basescan.org",
    "arbiscan.io",
    "www.arbiscan.io",
];

/// Normalise `raw` and classify it. See `plan/2-search.md` §12.1.
#[must_use]
pub fn classify_input(raw: &str) -> ClassifiedInput {
    let trimmed = strip_wrapping_quotes(raw.trim());
    let unwrapped = unwrap_explorer_url(trimmed).unwrap_or(trimmed);
    let classification = classify(unwrapped);
    // Hex inputs are canonicalised to lowercase so the TTL cache
    // does not treat `0xDEAD…` and `0xdead…` as different keys.
    let normalised = match &classification {
        Classification::Hash32 { lower_hex } | Classification::Address { lower_hex } => {
            lower_hex.clone()
        }
        Classification::EnsName(name) => name.clone(),
        _ => unwrapped.to_string(),
    };
    ClassifiedInput {
        normalised,
        classification,
    }
}

/// Peel off one pair of matching surrounding quotes. Handles ASCII
/// single / double quotes and the Unicode curly-quote pair.
fn strip_wrapping_quotes(input: &str) -> &str {
    let mut chars = input.chars();
    let Some(first) = chars.next() else {
        return input;
    };
    let last = chars.next_back();
    let Some(last) = last else {
        return input;
    };
    let matches = matches!(
        (first, last),
        ('"', '"') | ('\'', '\'') | ('\u{201C}', '\u{201D}'),
    );
    if matches {
        &input[first.len_utf8()..input.len() - last.len_utf8()]
    } else {
        input
    }
}

/// Recognise `https?://host/(tx|address|block)/{value}` and return the
/// trailing value. Returns `None` for anything else.
fn unwrap_explorer_url(input: &str) -> Option<&str> {
    let rest = input
        .strip_prefix("https://")
        .or_else(|| input.strip_prefix("http://"))?;

    let (host, path) = rest.split_once('/')?;
    if !EXPLORER_HOSTS
        .iter()
        .any(|h| h.eq_ignore_ascii_case(host))
    {
        return None;
    }

    for prefix in ["tx/", "address/", "block/"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            // Trim any trailing query string / fragment / path noise.
            let end = rest
                .find(|c: char| c == '/' || c == '?' || c == '#')
                .unwrap_or(rest.len());
            let value = &rest[..end];
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
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
