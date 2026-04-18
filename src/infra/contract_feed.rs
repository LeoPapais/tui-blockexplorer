//! Background task for the Contract Detail screen.
//!
//! Composes `AddressReaderPort` + `ProxyDetectionPort` to build the
//! Overview through the `load_contract_overview` use case, then
//! fires an `ContractSourcePort::get_source` call so the Source and
//! ABI tabs populate as soon as Etherscan answers.
//!
//! See `plan/7-contract-detail.md` sections 12.3 and 12.4.1.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::ContractFeedSender,
    application::{
        ports::{AddressReaderPort, ContractSourcePort, ProxyDetectionPort},
        use_cases::load_contract_overview,
    },
    domain::Chain,
};

pub fn spawn<A, P, S>(
    chain: Chain,
    address_reader: A,
    proxy_detector: P,
    source: S,
    sender: ContractFeedSender,
) -> JoinHandle<()>
where
    A: AddressReaderPort + Clone + 'static,
    P: ProxyDetectionPort + Clone + 'static,
    S: ContractSourcePort + Clone + 'static,
{
    tokio::spawn(async move {
        let ContractFeedSender {
            updates_tx,
            source_tx,
            mut input_rx,
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            // Overview + proxy detection run first because they are
            // the smaller calls.
            if let Ok(ov) =
                load_contract_overview::run(&address_reader, &proxy_detector, addr, chain).await
                && updates_tx.send(ov).is_err()
            {
                break;
            }
            // Etherscan source fetch: tolerate both `Ok(None)`
            // (unverified) and provider errors — the UI renders an
            // appropriate empty state without breaking the screen.
            if let Ok(Some(src)) = source.get_source(addr, chain).await
                && source_tx.send(src).is_err()
            {
                break;
            }
        }
    })
}
