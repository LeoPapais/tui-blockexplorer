//! Background task that answers Address requests with
//! `ContractOverview` values by composing an `AddressReaderPort` and
//! a `ProxyDetectionPort` through the `load_contract_overview` use
//! case.
//!
//! See `plan/7-contract-detail.md` section 12.3.

use tokio::task::JoinHandle;

use crate::{
    adapters::ui::ContractFeedSender,
    application::{
        ports::{AddressReaderPort, ProxyDetectionPort},
        use_cases::load_contract_overview,
    },
    domain::Chain,
};

pub fn spawn<A, P>(
    chain: Chain,
    address_reader: A,
    proxy_detector: P,
    sender: ContractFeedSender,
) -> JoinHandle<()>
where
    A: AddressReaderPort + 'static,
    P: ProxyDetectionPort + 'static,
{
    tokio::spawn(async move {
        let ContractFeedSender {
            updates_tx,
            mut input_rx,
        } = sender;
        while let Some(addr) = input_rx.recv().await {
            if let Ok(ov) =
                load_contract_overview::run(&address_reader, &proxy_detector, addr, chain)
                    .await
                && updates_tx.send(ov).is_err()
            {
                break;
            }
        }
    })
}
