//! Functional tests for `load_contract_events_page`.
//!
//! See `plan/7-contract-detail.md` section 12.5.3.

use blockexplorer_tui::{
    application::use_cases::load_contract_events_page::{self, DEFAULT_WINDOW_BLOCKS},
    domain::{Address, BlockNumber, Chain, DomainError, LogEntry, NetworkStatus, Wei},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::{StubEventLogPort, StubNetworkStatusPort};

fn contract() -> Address {
    Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
}

fn seed_head(status: &StubNetworkStatusPort, head: u64) {
    status.set_snapshot(NetworkStatus {
        chain: Chain::Ethereum,
        latest_block: BlockNumber::new(head),
        base_fee: Wei::new(0),
        block_time_avg_ms: 12_000,
    });
}

fn dummy_log() -> LogEntry {
    LogEntry {
        address: contract(),
        topics: Vec::new(),
        data: Vec::new(),
    }
}

#[tokio::test]
async fn it_reads_the_latest_window_when_offset_is_zero_and_head_hint_is_none() {
    let status = StubNetworkStatusPort::new();
    let event_log = StubEventLogPort::new();
    let head = 1_234_567u64;
    seed_head(&status, head);
    event_log.set_logs(contract(), vec![dummy_log(), dummy_log()]);

    let page =
        load_contract_events_page::run(&status, &event_log, contract(), Chain::Ethereum, None, 0)
            .await
            .expect("ok");

    assert_eq!(page.head.value(), head);
    assert_eq!(page.window_to.value(), head);
    assert_eq!(page.window_from.value(), head - (DEFAULT_WINDOW_BLOCKS - 1));
    assert_eq!(page.logs.len(), 2);
    assert!(page.has_older);
}

#[tokio::test]
async fn it_walks_backwards_by_offset_using_the_provided_head_hint() {
    let status = StubNetworkStatusPort::new();
    let event_log = StubEventLogPort::new();
    let head = 100_000u64;
    // head hint is provided, so NetworkStatusPort is never consulted
    // — proves the test does not hit the network.

    let page = load_contract_events_page::run(
        &status,
        &event_log,
        contract(),
        Chain::Ethereum,
        Some(BlockNumber::new(head)),
        3,
    )
    .await
    .expect("ok");

    assert_eq!(page.head.value(), head);
    assert_eq!(page.window_to.value(), head - 3 * DEFAULT_WINDOW_BLOCKS);
    assert_eq!(
        page.window_from.value(),
        head - 3 * DEFAULT_WINDOW_BLOCKS - (DEFAULT_WINDOW_BLOCKS - 1)
    );
    assert!(page.has_older);
}

#[tokio::test]
async fn it_caps_the_bottom_of_the_window_at_zero_and_flags_has_older_false() {
    let status = StubNetworkStatusPort::new();
    let event_log = StubEventLogPort::new();
    // Head = 2_000: one window covers 0..=2_000 (less than 5_000
    // blocks), has_older = false.
    let page = load_contract_events_page::run(
        &status,
        &event_log,
        contract(),
        Chain::Ethereum,
        Some(BlockNumber::new(2_000)),
        0,
    )
    .await
    .expect("ok");

    assert_eq!(page.window_from.value(), 0);
    assert_eq!(page.window_to.value(), 2_000);
    assert!(!page.has_older);
}

#[tokio::test]
async fn it_returns_empty_page_when_offset_exceeds_chain_height() {
    let status = StubNetworkStatusPort::new();
    let event_log = StubEventLogPort::new();
    let page = load_contract_events_page::run(
        &status,
        &event_log,
        contract(),
        Chain::Ethereum,
        Some(BlockNumber::new(1_000)),
        5,
    )
    .await
    .expect("ok");

    assert!(page.logs.is_empty());
    assert!(!page.has_older);
}

#[tokio::test]
async fn it_propagates_network_status_errors_when_head_hint_is_missing() {
    let status = StubNetworkStatusPort::new();
    let event_log = StubEventLogPort::new();
    status.set_broken(true);

    let err =
        load_contract_events_page::run(&status, &event_log, contract(), Chain::Ethereum, None, 0)
            .await
            .expect_err("broken network status must propagate");

    assert!(matches!(err, DomainError::ProviderUnavailable));
}

#[tokio::test]
async fn it_propagates_event_log_errors() {
    // EventLog stub does not support injected errors; we simulate
    // the failure path with a tiny inline adapter that always
    // returns Err.
    use blockexplorer_tui::application::ports::{BlockRange, EventLogPort};
    use blockexplorer_tui::domain::{Chain, DomainError, LogEntry};

    #[derive(Clone, Default)]
    struct Broken;
    impl EventLogPort for Broken {
        async fn get_logs(
            &self,
            _address: Address,
            _chain: Chain,
            _range: BlockRange,
        ) -> Result<Vec<LogEntry>, DomainError> {
            Err(DomainError::ProviderUnavailable)
        }
    }

    let status = StubNetworkStatusPort::new();
    let page = load_contract_events_page::run(
        &status,
        &Broken,
        contract(),
        Chain::Ethereum,
        Some(BlockNumber::new(100_000)),
        0,
    )
    .await;

    assert!(matches!(page, Err(DomainError::ProviderUnavailable)));
}

#[tokio::test]
async fn network_status_is_not_called_when_head_hint_is_provided() {
    let status = StubNetworkStatusPort::new();
    // status is deliberately broken — if the use case consulted it
    // we would surface an error; instead we pass a head hint so
    // NetworkStatusPort is never invoked.
    status.set_broken(true);
    let event_log = StubEventLogPort::new();

    let page = load_contract_events_page::run(
        &status,
        &event_log,
        contract(),
        Chain::Ethereum,
        Some(BlockNumber::new(10_000)),
        0,
    )
    .await
    .expect("broken status must not be consulted when head_hint is Some");

    assert_eq!(page.head.value(), 10_000);
}
