//! Wiremock test for the Alchemy event-log adapter.
//!
//! See `plan/7-contract-detail.md` section 12.4.3.

use blockexplorer_tui::{
    adapters::rpc::{AlchemyEventLog, RpcClient},
    application::ports::{BlockRange, EventLogPort},
    domain::{Address, BlockNumber, Chain},
};
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::method,
};

use crate::support::fixture_loader::load_text;

fn adapter_for(url: &str) -> AlchemyEventLog {
    let rpc = RpcClient::new(Url::parse(url).unwrap(), reqwest::Client::new());
    AlchemyEventLog::new(rpc)
}

#[tokio::test]
async fn decodes_log_rows_from_eth_get_logs() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("alchemy__eth_getLogs__transfer.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let adapter = adapter_for(&server.uri());
    let contract = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    let range = BlockRange {
        from: BlockNumber::new(0x1406f00),
        to: BlockNumber::new(0x1406f50),
    };

    let logs = adapter
        .get_logs(contract, Chain::Ethereum, range)
        .await
        .expect("ok");

    assert_eq!(logs.len(), 1);
    let log = &logs[0];
    assert_eq!(log.address.to_hex(), "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    assert_eq!(log.topics.len(), 3);
    // Transfer(address,address,uint256) topic0.
    let topic0 = hex::encode(log.topics[0]);
    assert_eq!(
        topic0,
        "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
    );
    // data = 1_000_000 padded to 32 bytes
    assert_eq!(log.data.len(), 32);
    assert_eq!(log.data[31 - 2], 0x0f);
    assert_eq!(log.data[31 - 1], 0x42);
    assert_eq!(log.data[31], 0x40);
}
