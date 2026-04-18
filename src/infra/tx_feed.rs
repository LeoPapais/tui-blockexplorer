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
        ports::{ContractSourcePort, SignatureDirectoryPort, TxReaderPort},
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

/// Spawn the feed with ABI + signature-directory decoding enabled.
pub fn spawn_with_decoding<R, C, S>(
    chain: Chain,
    reader: R,
    contract_source: C,
    signatures: S,
    sender: TxFeedSender,
) -> JoinHandle<()>
where
    R: TxReaderPort + 'static,
    C: ContractSourcePort + 'static,
    S: SignatureDirectoryPort + 'static,
{
    tokio::spawn(async move {
        let TxFeedSender {
            updates_tx,
            mut input_rx,
        } = sender;
        while let Some(hash) = input_rx.recv().await {
            let result = load_tx_overview::run_with_decoding(
                &reader,
                &contract_source,
                &signatures,
                hash,
                chain,
            )
            .await;
            let view = match result {
                Ok(view) => view,
                Err(_) => continue,
            };
            // Silently carry on on send failure (screen dropped).
            if !send_or_break(&updates_tx, view) {
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
