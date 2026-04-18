//! Background task for the Contract Detail screen.
//!
//! Composes `AddressReaderPort` + `ProxyDetectionPort` for the
//! Overview, fires `ContractSourcePort::get_source` for the Source /
//! ABI tabs, and answers on-demand requests from the Read / Events /
//! Storage tabs.
//!
//! See `plan/7-contract-detail.md` sections 12.3, 12.4.1, 12.4.2 and
//! 12.4.3.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::ContractFeedSender,
    application::{
        ports::{
            AddressReaderPort, ContractReaderPort, ContractSourcePort, EventLogPort,
            ProxyDetectionPort, StoragePort,
        },
        use_cases::load_contract_overview,
    },
    domain::{Address, Chain},
};

#[allow(clippy::too_many_arguments)]
pub fn spawn<A, P, S, R, E, St>(
    chain: Chain,
    address_reader: A,
    proxy_detector: P,
    source: S,
    reader: R,
    event_log: E,
    storage: St,
    sender: ContractFeedSender,
) -> JoinHandle<()>
where
    A: AddressReaderPort + Clone + 'static,
    P: ProxyDetectionPort + Clone + 'static,
    S: ContractSourcePort + Clone + 'static,
    R: ContractReaderPort + Clone + 'static,
    E: EventLogPort + Clone + 'static,
    St: StoragePort + Clone + 'static,
{
    tokio::spawn(async move {
        let ContractFeedSender {
            updates_tx,
            source_tx,
            mut read_rx,
            read_tx,
            mut events_rx,
            events_tx,
            mut storage_rx,
            storage_tx,
            mut input_rx,
        } = sender;

        let mut active: Option<Address> = None;

        loop {
            tokio::select! {
                addr = input_rx.recv() => {
                    let Some(addr) = addr else { break };
                    active = Some(addr);
                    if let Ok(ov) = load_contract_overview::run(
                        &address_reader,
                        &proxy_detector,
                        addr,
                        chain,
                    )
                    .await
                        && updates_tx.send(ov).is_err()
                    {
                        break;
                    }
                    if let Ok(Some(src)) = source.get_source(addr, chain).await
                        && source_tx.send(src).is_err()
                    {
                        break;
                    }
                }
                req = read_rx.recv() => {
                    let Some(req) = req else { break };
                    let Some(address) = active else { continue };
                    let result = reader
                        .call(address, chain, &req.function, req.args)
                        .await;
                    if read_tx.send(result).is_err() {
                        break;
                    }
                }
                req = events_rx.recv() => {
                    let Some(req) = req else { break };
                    let Some(address) = active else { continue };
                    let result = event_log.get_logs(address, chain, req.range).await;
                    if events_tx.send(result).is_err() {
                        break;
                    }
                }
                req = storage_rx.recv() => {
                    let Some(req) = req else { break };
                    let Some(address) = active else { continue };
                    let result = storage.get_at(address, chain, req.slot).await;
                    if storage_tx.send(result).is_err() {
                        break;
                    }
                }
            }
        }
    })
}
