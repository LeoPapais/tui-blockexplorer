//! Background task for the Contract Detail screen.
//!
//! Composes `AddressReaderPort` + `ProxyDetectionPort` for the
//! Overview, fires `ContractSourcePort::get_source` for the Source /
//! ABI tabs, and answers on-demand `eth_call` requests from the Read
//! tab via `ContractReaderPort`.
//!
//! See `plan/7-contract-detail.md` sections 12.3, 12.4.1 and 12.4.2.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::ContractFeedSender,
    application::{
        ports::{AddressReaderPort, ContractReaderPort, ContractSourcePort, ProxyDetectionPort},
        use_cases::load_contract_overview,
    },
    domain::{Address, Chain},
};

pub fn spawn<A, P, S, R>(
    chain: Chain,
    address_reader: A,
    proxy_detector: P,
    source: S,
    reader: R,
    sender: ContractFeedSender,
) -> JoinHandle<()>
where
    A: AddressReaderPort + Clone + 'static,
    P: ProxyDetectionPort + Clone + 'static,
    S: ContractSourcePort + Clone + 'static,
    R: ContractReaderPort + Clone + 'static,
{
    tokio::spawn(async move {
        let ContractFeedSender {
            updates_tx,
            source_tx,
            mut read_rx,
            read_tx,
            mut input_rx,
        } = sender;

        // State carried across iterations: once we know which
        // address the screen is looking at, every Read request is
        // resolved against that same address.
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
            }
        }
    })
}
