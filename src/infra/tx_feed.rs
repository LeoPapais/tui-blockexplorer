//! Background task that answers `TxHash` requests with enriched
//! `TxView` values. Mirrors `block_feed.rs` and `search_feed.rs` for
//! the TxDetail screen.
//!
//! Every live caller uses [`spawn_full`]: the decoding happens
//! up-front and the heavier asset-change / state-diff tabs populate
//! concurrently so none of the tabs stays stuck in "Pending..."
//! indefinitely. Composite adapters (Etherscan or Noop,
//! Sourcify or Noop) let the caller opt out of individual decoders
//! without having to pick a different spawn path.
//!
//! See `plan/4-tx-detail.md` sections 12.3, 12.4.2 and 12.4.3, plus
//! `plan/15-backlog.md` section 3.3 for the proxy-following cascade.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::TxFeedSender,
    application::{
        TxView,
        ports::{
            ContractSourcePort, ProxyDetectionPort, SignatureDirectoryPort, TxReaderPort,
            TxSimulationPort, TxTracePort,
        },
        use_cases::load_tx_overview,
    },
    domain::Chain,
};

fn send_or_break(tx: &tokio::sync::mpsc::UnboundedSender<TxView>, view: TxView) -> bool {
    tx.send(view).is_ok()
}

/// Spawn the full pipeline: decoding + asset-changes simulation +
/// state-diff trace. The enriched view is delivered twice so the
/// UI renders the decoded overview immediately and the heavier
/// tabs populate as soon as the downstream adapters return.
#[allow(clippy::too_many_arguments)]
pub fn spawn_full<R, C, S, P, Sim, Trace>(
    chain: Chain,
    reader: R,
    contract_source: C,
    signatures: S,
    proxy_detector: P,
    sim: Sim,
    trace: Trace,
    sender: TxFeedSender,
) -> JoinHandle<()>
where
    R: TxReaderPort + 'static,
    C: ContractSourcePort + 'static,
    S: SignatureDirectoryPort + 'static,
    P: ProxyDetectionPort + 'static,
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
                &proxy_detector,
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
