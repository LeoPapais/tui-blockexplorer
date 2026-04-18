//! Background task that answers search input with resolved candidate
//! lists. Mirrors `home_feed.rs` but for the Search screen.
//!
//! Emission is two-phase:
//!
//! 1. `ResolveQuery::run` produces the base candidates (Address,
//!    Contract shortcut, Tx, Block, Token ticker, ...). This update
//!    is published immediately.
//! 2. When the base list contains at least one `Contract` shortcut,
//!    the feed fires a follow-up `TokenReaderPort::get` for that
//!    address. If `Ok(Some(overview))`, a second update is published
//!    with the same input and a `Token(metadata)` candidate appended.
//!
//! The probe is **pessimistic**: it runs only when we already have
//! evidence that the address is a contract (EOAs pay nothing), and
//! is skipped entirely for non-address inputs (tx / block / ENS
//! names resolving to EOAs / bare tickers / free text).
//!
//! See `plan/2-search.md` section 11.

use tokio::task::JoinHandle;

use crate::{
    adapters::{
        cache::TtlCache,
        clock::SystemClock,
        ui::{SearchFeedSender, SearchFeedUpdate},
    },
    application::{
        ports::{
            AddressLookupPort, BlockLookupPort, Clock, EnsResolverPort, TokenReaderPort,
            TokenSearchPort, TxLookupPort,
        },
        use_cases::resolve_query::{ClassifiedInput, ResolveQuery, classify_input},
    },
    domain::{Address, Chain, ResolvedEntity},
};

/// Alias for the TTL cache used by the search feed. Keyed by the
/// active chain plus the normalised input, value is the last enriched
/// candidate list we published for that key. See `plan/2-search.md`
/// section 12.4.
pub type SearchCache<C = SystemClock> = TtlCache<(Chain, String), Vec<ResolvedEntity>, C>;

/// Spawn the resolver task with six ports. The extra `token_reader`
/// (compared with the pre-ERC20-probe shape of `plan/2-search.md`)
/// backs the optional follow-up emission described above.
#[allow(clippy::too_many_arguments)]
pub fn spawn<B, T, A, E, S, K>(
    chain: Chain,
    block: B,
    tx: T,
    address: A,
    ens: E,
    token: S,
    token_reader: K,
    sender: SearchFeedSender,
) -> JoinHandle<()>
where
    B: BlockLookupPort + 'static,
    T: TxLookupPort + 'static,
    A: AddressLookupPort + 'static,
    E: EnsResolverPort + 'static,
    S: TokenSearchPort + 'static,
    K: TokenReaderPort + 'static,
{
    spawn_with_cache::<_, _, _, _, _, _, SystemClock>(
        chain,
        block,
        tx,
        address,
        ens,
        token,
        token_reader,
        sender,
        None,
    )
}

/// Spawn the resolver task with an optional 60 s TTL cache keyed by
/// `(chain, normalised_input)`. When `cache` is `Some`, a cache hit
/// skips both the base resolution and the ERC-20 probe; a cache miss
/// runs the full two-phase emission and writes the final list back
/// into the cache for future hits. See `plan/2-search.md` §12.4.
#[allow(clippy::too_many_arguments)]
pub fn spawn_with_cache<B, T, A, E, S, K, C>(
    chain: Chain,
    block: B,
    tx: T,
    address: A,
    ens: E,
    token: S,
    token_reader: K,
    sender: SearchFeedSender,
    cache: Option<SearchCache<C>>,
) -> JoinHandle<()>
where
    B: BlockLookupPort + 'static,
    T: TxLookupPort + 'static,
    A: AddressLookupPort + 'static,
    E: EnsResolverPort + 'static,
    S: TokenSearchPort + 'static,
    K: TokenReaderPort + 'static,
    C: Clock + 'static,
{
    tokio::spawn(async move {
        let SearchFeedSender {
            updates_tx,
            mut input_rx,
        } = sender;

        let query = ResolveQuery {
            block: &block,
            tx: &tx,
            address: &address,
            ens: &ens,
            token: &token,
        };

        while let Some(input) = input_rx.recv().await {
            let trimmed = input.trim();
            if trimmed.is_empty() {
                if updates_tx
                    .send(SearchFeedUpdate {
                        input: input.clone(),
                        candidates: Vec::new(),
                    })
                    .is_err()
                {
                    break;
                }
                continue;
            }

            let ClassifiedInput { normalised, .. } = classify_input(trimmed);
            let cache_key = (chain, normalised.clone());

            if let Some(cache) = cache.as_ref()
                && let Some(cached) = cache.get(&cache_key).await
            {
                if updates_tx
                    .send(SearchFeedUpdate {
                        input,
                        candidates: cached,
                    })
                    .is_err()
                {
                    break;
                }
                continue;
            }

            let base_candidates = query.run(trimmed, chain).await.unwrap_or_default();

            if updates_tx
                .send(SearchFeedUpdate {
                    input: input.clone(),
                    candidates: base_candidates.clone(),
                })
                .is_err()
            {
                break;
            }

            // Phase 2 — ERC-20 probe. Only reached when the base
            // list contains a Contract shortcut, so EOAs and non-
            // address inputs never hit the TokenReaderPort.
            let final_candidates = if let Some(contract_addr) =
                first_contract_address(&base_candidates)
                && let Ok(Some(overview)) = token_reader.get(contract_addr, chain).await
            {
                let mut enriched = base_candidates.clone();
                enriched.push(ResolvedEntity::Token(overview.metadata));
                if updates_tx
                    .send(SearchFeedUpdate {
                        input: input.clone(),
                        candidates: enriched.clone(),
                    })
                    .is_err()
                {
                    break;
                }
                enriched
            } else {
                base_candidates
            };

            // Write-back. Skip caching when the only row is NotFound
            // so transient errors are not sticky for the full TTL.
            let cache_worthy = !final_candidates.is_empty()
                && !final_candidates
                    .iter()
                    .all(|c| matches!(c, ResolvedEntity::NotFound { .. }));
            if cache_worthy && let Some(cache) = cache.as_ref() {
                cache.insert(cache_key, final_candidates).await;
            }
        }
    })
}

fn first_contract_address(candidates: &[ResolvedEntity]) -> Option<Address> {
    candidates.iter().find_map(|c| match c {
        ResolvedEntity::Contract { address } => Some(*address),
        _ => None,
    })
}
