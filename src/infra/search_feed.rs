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
    adapters::ui::{SearchFeedSender, SearchFeedUpdate},
    application::{
        ports::{
            AddressLookupPort, BlockLookupPort, EnsResolverPort, TokenReaderPort, TokenSearchPort,
            TxLookupPort,
        },
        use_cases::resolve_query::ResolveQuery,
    },
    domain::{Address, Chain, ResolvedEntity},
};

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
            let base_candidates = if trimmed.is_empty() {
                Vec::new()
            } else {
                query.run(trimmed, chain).await.unwrap_or_default()
            };

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
            if let Some(contract_addr) = first_contract_address(&base_candidates)
                && let Ok(Some(overview)) = token_reader.get(contract_addr, chain).await
            {
                let mut enriched = base_candidates.clone();
                enriched.push(ResolvedEntity::Token(overview.metadata));
                if updates_tx
                    .send(SearchFeedUpdate {
                        input,
                        candidates: enriched,
                    })
                    .is_err()
                {
                    break;
                }
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
