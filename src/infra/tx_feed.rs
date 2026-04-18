//! Background task that answers `TxHash` requests with enriched
//! `TxView` values. Mirrors `block_feed.rs` and `search_feed.rs` for
//! the TxDetail screen.
//!
//! Decoding is optional: callers can supply concrete
//! `ContractSourcePort` / `SignatureDirectoryPort` implementations for
//! ABI + signature directory fallback, or pass `None` to fall back to
//! the bare-view path that leaves the method / logs undecoded.
//!
//! See `plan/4-tx-detail.md` sections 12.3 and 12.4.2.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::TxFeedSender,
    application::{
        TxView,
        ports::{
            ContractSourcePort, SignatureDirectoryPort, TxReaderPort, TxSimulationPort,
            TxTracePort,
        },
        use_cases::load_tx_overview,
    },
    domain::Chain,
};

/// Spawn the feed without decoding: method + logs come through
/// undecoded. Used by paths that do not yet wire ABI / signature
/// adapters.
pub fn spawn<R>(chain: Chain, reader: R, sender: TxFeedSender) -> JoinHandle<()>
where
    R: TxReaderPort + 'static,
{
    tokio::spawn(async move {
        let TxFeedSender {
            updates_tx,
            mut input_rx,
        } = sender;
        while let Some(hash) = input_rx.recv().await {
            if let Ok(view) = load_tx_overview::run(&reader, hash, chain).await
                && updates_tx.send(view).is_err()
            {
                break;
            }
        }
    })
}

fn send_or_break(
    tx: &tokio::sync::mpsc::UnboundedSender<TxView>,
    view: TxView,
) -> bool {
    tx.send(view).is_ok()
}

/// Spawn the full pipeline: decoding + asset-changes simulation +
/// state-diff trace. The enriched view is delivered twice so the
/// UI renders the decoded overview immediately and the heavier
/// tabs populate as soon as the downstream adapters return.
pub fn spawn_full<R, C, S, Sim, Trace>(
    chain: Chain,
    reader: R,
    contract_source: C,
    signatures: S,
    sim: Sim,
    trace: Trace,
    sender: TxFeedSender,
) -> JoinHandle<()>
where
    R: TxReaderPort + 'static,
    C: ContractSourcePort + 'static,
    S: SignatureDirectoryPort + 'static,
    Sim: TxSimulationPort + Clone + 'static,
    Trace: TxTracePort + Clone + 'static,
{
    tokio::spawn(async move {
        let TxFeedSender {
            updates_tx,
            mut input_rx,
        } = sender;

        while let Some(hash) = input_rx.recv().await {
            let Ok(mut view) = load_tx_overview::run_with_decoding(
                &reader,
                &contract_source,
                &signatures,
                hash,
                chain,
            )
            .await
            else {
                continue;
            };

            if !send_or_break(&updates_tx, view.clone()) {
                break;
            }

            // Run the heavier enrichments concurrently.
            let sim_clone = sim.clone();
            let trace_clone = trace.clone();
            let view_for_sim = &mut view;

            let (sim_status, trace_status) = tokio::join!(
                async {
                    let mut v = view_for_sim.clone();
                    load_tx_overview::load_asset_changes(&sim_clone, &mut v, chain).await;
                    v.asset_changes
                },
                async {
                    let mut v = view_for_sim.clone();
                    load_tx_overview::load_state_diff(&trace_clone, &mut v, chain).await;
                    v.state_diff
                },
            );

            view.asset_changes = sim_status;
            view.state_diff = trace_status;

            if !send_or_break(&updates_tx, view) {
                break;
            }
        }
    })
}
