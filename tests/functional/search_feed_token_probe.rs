//! Functional tests for the ERC-20 probe inside the search feed.
//!
//! Asserts on:
//!
//! - EOA inputs produce a single update and zero calls to
//!   `TokenReaderPort::get` (pessimistic contract).
//! - Contract-but-not-ERC20 inputs produce a single update with the
//!   Contract shortcut and exactly one TokenReader probe (which
//!   returns None).
//! - Contract-that-is-ERC20 inputs produce two updates: the base
//!   list first, then the enriched list with a `Token` candidate.
//!
//! See `plan/2-search.md` section 11.

use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::search_feed,
    domain::{Address, AddressKind, Chain, ResolvedEntity, TokenMetadata, TokenOverview},
    infra::search_feed as feed_task,
};
use pretty_assertions::assert_eq;
use tokio::time::{sleep, timeout};

use crate::support::stubs::{
    StubAddressLookupPort, StubBlockLookupPort, StubEnsResolverPort, StubTokenReaderPort,
    StubTokenSearchPort, StubTxLookupPort,
};

const EOA_HEX: &str = "0xd8da6bf26964af9d7eed9e03e53415d37aa96045";
const CONTRACT_HEX: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";

fn addr(hex: &str) -> Address {
    Address::from_hex(hex).unwrap()
}

fn usdc_overview() -> TokenOverview {
    TokenOverview {
        metadata: TokenMetadata {
            address: addr(CONTRACT_HEX),
            symbol: "USDC".into(),
            name: "USD Coin".into(),
            decimals: 6,
        },
        total_supply: 0,
        price: None,
    }
}

/// Spawn a freshly wired search feed and drive a single input
/// through it. Returns the collected updates plus the token-reader
/// stub so the test can assert on its call counter.
async fn drive_one_input(
    input: &str,
    address_stub: StubAddressLookupPort,
    token_reader: StubTokenReaderPort,
) -> (Vec<Vec<ResolvedEntity>>, StubTokenReaderPort) {
    let block = StubBlockLookupPort::new();
    let tx = StubTxLookupPort::new();
    let ens = StubEnsResolverPort::new();
    let token_search = StubTokenSearchPort::new();

    let (feed, sender) = search_feed();
    let handle = feed_task::spawn(
        Chain::Ethereum,
        block,
        tx,
        address_stub,
        ens,
        token_search,
        token_reader.clone(),
        sender,
    );

    feed.input_tx.send(input.to_string()).unwrap();

    // Wait up to 500 ms for the first update; then drain any
    // follow-up updates in a 200 ms window so the second emission
    // (if it happens) has time to land.
    let mut rx = feed.updates_rx;
    let mut updates: Vec<Vec<ResolvedEntity>> = Vec::new();
    if let Ok(Some(first)) = timeout(Duration::from_millis(500), rx.recv()).await {
        updates.push(first.candidates);
    }
    sleep(Duration::from_millis(200)).await;
    while let Ok(update) = rx.try_recv() {
        updates.push(update.candidates);
    }

    drop(feed.input_tx);
    let _ = handle.await;
    (updates, token_reader)
}

#[tokio::test]
async fn eoa_input_never_probes_token_reader() {
    let address_stub = StubAddressLookupPort::new();
    address_stub.set_kind(addr(EOA_HEX), AddressKind::Eoa { delegated_to: None });
    let token_reader = StubTokenReaderPort::new();

    let (updates, reader) = drive_one_input(EOA_HEX, address_stub, token_reader).await;

    assert_eq!(updates.len(), 1, "EOA must produce a single update");
    assert_eq!(reader.call_count(), 0, "EOA must not trigger a probe");
    assert!(
        updates[0].iter().all(|c| !matches!(
            c,
            ResolvedEntity::Contract { .. } | ResolvedEntity::Token(_)
        )),
        "EOA update must not include Contract/Token shortcuts",
    );
}

#[tokio::test]
async fn contract_without_metadata_probes_once_and_emits_only_base_update() {
    let address_stub = StubAddressLookupPort::new();
    address_stub.set_kind(addr(CONTRACT_HEX), AddressKind::Contract);
    let token_reader = StubTokenReaderPort::new();
    // Intentionally do NOT insert a TokenOverview: the probe will
    // return `Ok(None)` and the feed must not emit a second update.

    let (updates, reader) = drive_one_input(CONTRACT_HEX, address_stub, token_reader).await;

    assert_eq!(
        updates.len(),
        1,
        "no second update when the probe returns None",
    );
    assert_eq!(
        reader.call_count(),
        1,
        "contract triggers exactly one probe"
    );
    let base = &updates[0];
    assert!(matches!(base.first(), Some(ResolvedEntity::Address { .. })));
    assert!(
        base.iter()
            .any(|c| matches!(c, ResolvedEntity::Contract { .. }))
    );
    assert!(
        !base.iter().any(|c| matches!(c, ResolvedEntity::Token(_))),
        "contract without ERC-20 metadata must not gain a Token row",
    );
}

#[tokio::test]
async fn erc20_contract_emits_two_updates_with_token_appended() {
    let address_stub = StubAddressLookupPort::new();
    address_stub.set_kind(addr(CONTRACT_HEX), AddressKind::Contract);
    let token_reader = StubTokenReaderPort::new();
    token_reader.insert(usdc_overview());

    let (updates, reader) = drive_one_input(CONTRACT_HEX, address_stub, token_reader).await;

    assert_eq!(reader.call_count(), 1);
    assert_eq!(updates.len(), 2, "expected base + enriched update");

    let base = &updates[0];
    assert!(
        !base.iter().any(|c| matches!(c, ResolvedEntity::Token(_))),
        "first update must not contain the Token row",
    );

    let enriched = &updates[1];
    assert!(
        matches!(enriched.first(), Some(ResolvedEntity::Address { .. })),
        "Address row stays first even after the probe completes",
    );
    assert!(
        enriched
            .iter()
            .any(|c| matches!(c, ResolvedEntity::Contract { .. })),
        "Contract shortcut remains in the enriched list",
    );
    assert!(
        matches!(
            enriched.last(),
            Some(ResolvedEntity::Token(meta)) if meta.symbol == "USDC",
        ),
        "Token row must be appended at the end of the enriched list",
    );
}
