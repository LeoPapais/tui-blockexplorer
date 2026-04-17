//! Background task that answers search input with resolved candidate
//! lists. Mirrors `home_feed.rs` but for the Search screen.
//!
//! See `plan/2-search.md` section 10.3.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::{SearchFeedSender, SearchFeedUpdate},
    application::{
        ports::{
            AddressLookupPort, BlockLookupPort, EnsResolverPort, TokenSearchPort, TxLookupPort,
        },
        use_cases::resolve_query::ResolveQuery,
    },
    domain::Chain,
};

/// Spawn a resolver task that owns `sender` and drives
/// [`ResolveQuery`] against the provided ports. The task exits cleanly
/// when the screen drops its input sender (closing `input_rx`).
pub fn spawn<B, T, A, E, S>(
    chain: Chain,
    block: B,
    tx: T,
    address: A,
    ens: E,
    token: S,
    sender: SearchFeedSender,
) -> JoinHandle<()>
where
    B: BlockLookupPort + 'static,
    T: TxLookupPort + 'static,
    A: AddressLookupPort + 'static,
    E: EnsResolverPort + 'static,
    S: TokenSearchPort + 'static,
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
            let candidates = if trimmed.is_empty() {
                Vec::new()
            } else {
                query.run(trimmed, chain).await.unwrap_or_default()
            };
            if updates_tx
                .send(SearchFeedUpdate {
                    input: input.clone(),
                    candidates,
                })
                .is_err()
            {
                break;
            }
        }
    })
}
