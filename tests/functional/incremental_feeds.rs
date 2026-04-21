//! Incremental feed ordering (plan/18 Slice E).
//!
//! Address fan-out must emit `AddressOverview` before slower peers
//! finish (`plan/16-unified-address-detail.md` §5). The transaction feed
//! must emit a bare `TxView` before decoding completes when lookups
//! are artificially slow.

use std::sync::Arc;
use std::time::Duration;

use blockexplorer_tui::{
    adapters::ui::{address_feed, tx_feed},
    application::{LoadStatus, ports::PortfolioPort},
    domain::{
        Address, AddressKind, AddressOverview, BlockHash, BlockNumber, Chain, DomainError,
        PriceLookup, TokenHolding, TokenMetadata, Transaction, TxHash, TxStatus, TxType, Wei,
    },
    infra::address_feed as address_feed_task,
    infra::tx_feed as tx_feed_task,
};
use pretty_assertions::assert_eq;
use tokio::sync::Barrier;

use crate::support::stubs::{
    StubAccountTransactionsPort, StubAddressReaderPort, StubContractReaderPort,
    StubContractSourcePort, StubEnsResolverPort, StubEventLogPort, StubNetworkStatusPort,
    StubPortfolioPort, StubPricesPort, StubProxyDetectionPort, StubSignatureDirectoryPort,
    StubStoragePort, StubTokenPriceStreamPort, StubTokenReaderPort, StubTransfersPort,
    StubTxReaderPort, StubTxSimulationPort, StubTxTracePort,
};

#[derive(Clone)]
struct BarrierPortfolio {
    inner: StubPortfolioPort,
    gate: Arc<Barrier>,
}

impl PortfolioPort for BarrierPortfolio {
    async fn get_token_balances(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<Vec<TokenHolding>, DomainError> {
        self.gate.wait().await;
        self.inner.get_token_balances(address, chain).await
    }
}

fn sample_eoa_overview(addr: Address) -> AddressOverview {
    AddressOverview {
        chain: Chain::Ethereum,
        address: addr,
        balance: Wei::new(1u128),
        nonce: 0,
        kind: AddressKind::Eoa { delegated_to: None },
        delegated_to: None,
        ens_name: None,
    }
}

#[tokio::test]
async fn address_feed_sends_overview_before_gated_portfolio_completes() {
    let addr = Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap();
    let chain = Chain::Ethereum;

    let reader = StubAddressReaderPort::new();
    let ov = sample_eoa_overview(addr);
    reader.insert(ov.clone());

    let transfers = StubTransfersPort::new();
    let account_transactions = StubAccountTransactionsPort::new();
    let portfolio_inner = StubPortfolioPort::new();
    let meta = TokenMetadata {
        address: Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        symbol: "USDC".to_string(),
        name: "USD Coin".to_string(),
        decimals: 6,
    };
    portfolio_inner.set_holdings(
        addr,
        vec![TokenHolding {
            metadata: meta,
            balance: Wei::new(1_000_000u128),
            price: PriceLookup::Pending,
        }],
    );

    let gate = Arc::new(Barrier::new(2));
    let portfolio = BarrierPortfolio {
        inner: portfolio_inner,
        gate: Arc::clone(&gate),
    };

    let (mut feed, sender) = address_feed();
    let _handle = address_feed_task::spawn(
        chain,
        reader,
        transfers,
        account_transactions,
        portfolio,
        StubTokenReaderPort::new(),
        StubPricesPort::new(),
        StubEnsResolverPort::new(),
        StubProxyDetectionPort::new(),
        StubContractSourcePort::new(),
        StubContractReaderPort::new(),
        StubEventLogPort::new(),
        StubStoragePort::new(),
        StubNetworkStatusPort::new(),
        StubTokenPriceStreamPort::new(),
        sender,
    );

    feed.input_tx.send(addr).expect("send address");

    let first = feed
        .updates_rx
        .recv()
        .await
        .expect("overview must arrive without waiting on portfolio");
    assert_eq!(first.address, addr);

    assert!(
        tokio::time::timeout(Duration::from_millis(200), feed.portfolio_rx.recv())
            .await
            .is_err(),
        "portfolio must not complete until the barrier releases the gated port"
    );

    gate.wait().await;

    let holdings = feed
        .portfolio_rx
        .recv()
        .await
        .expect("portfolio after gate");
    assert_eq!(holdings.len(), 1);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn tx_feed_sends_bare_view_before_decoding_when_signatures_slow() {
    let chain = Chain::Ethereum;
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();
    let detector = StubProxyDetectionPort::new();
    let sim = StubTxSimulationPort::new();
    let tracer = StubTxTracePort::new();

    let tx = Transaction {
        chain,
        hash: TxHash::from_hex(
            "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944c",
        )
        .unwrap(),
        status: TxStatus::Success,
        block_number: Some(BlockNumber::new(18_000_000)),
        block_hash: Some(
            BlockHash::from_hex(
                "0xbbbb000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
        ),
        tx_index: Some(0),
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()),
        value: Wei::new(0),
        gas_price: Wei::new(1u128),
        gas_used: Some(21_000),
        gas_limit: 50_000,
        nonce: 1,
        tx_type: TxType::Legacy,
        input: vec![0xa9, 0x05, 0x9c, 0xbb],
        logs: Vec::new(),
        raw_json: "{}".to_string(),
    };
    reader.insert(tx.clone());

    signatures.set_selector([0xa9, 0x05, 0x9c, 0xbb], "transfer(address,uint256)");
    signatures.set_delay(Duration::from_secs(60));

    let (mut feed, sender) = tx_feed();
    let _task = tx_feed_task::spawn_full(
        chain,
        reader.clone(),
        contract_source,
        signatures,
        detector,
        sim,
        tracer,
        sender,
    );

    feed.input_tx.send(tx.hash).expect("hash");

    let first = feed.updates_rx.recv().await.expect("bare view");
    assert!(
        first.decoded_method.is_none(),
        "bare view must not wait on signature directory"
    );
    assert_eq!(first.tx.hash, tx.hash);

    tokio::time::advance(Duration::from_secs(60)).await;

    let second = feed.updates_rx.recv().await.expect("decoded view");
    assert!(
        second.decoded_method.is_some(),
        "after decoding, method row should be populated"
    );

    let third = feed.updates_rx.recv().await.expect("enriched view");
    assert!(
        !matches!(third.asset_changes, LoadStatus::Pending),
        "final push should reflect simulator / tracer completion"
    );
}
