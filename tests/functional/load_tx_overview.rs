//! Functional tests for the `load_tx_overview` use case.
//!
//! See `plan/4-tx-detail.md` section 12.1.

use blockexplorer_tui::{
    application::{LoadStatus, SignatureSource, TxView, use_cases::load_tx_overview},
    domain::{
        Address, AddressStateDiff, AssetChange, AssetChangeKind, AssetKind, BlockHash, BlockNumber,
        Chain, ContractAbi, DiffChange, DomainError, LogEntry, ProxyInfo, ProxyKind, StateDiff,
        Transaction, TxHash, TxStatus, TxType, Wei,
    },
};
use pretty_assertions::assert_eq;

use crate::support::stubs::{
    StubContractSourcePort, StubProxyDetectionPort, StubSignatureDirectoryPort, StubTxReaderPort,
    StubTxSimulationPort, StubTxTracePort,
};

fn base_tx(hash_hex: &str) -> Transaction {
    Transaction {
        chain: Chain::Ethereum,
        hash: TxHash::from_hex(hash_hex).unwrap(),
        status: TxStatus::Success,
        block_number: Some(BlockNumber::new(21_000_000)),
        block_hash: Some(
            BlockHash::from_hex(
                "0xaaaa000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
        ),
        tx_index: Some(3),
        from: Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
        to: Some(Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()),
        value: Wei::new(0),
        gas_price: Wei::new(14_000_000_000),
        gas_used: Some(52_341),
        gas_limit: 80_000,
        nonce: 42,
        tx_type: TxType::DynamicFee,
        input: vec![0xa9, 0x05, 0x9c, 0xbb], // transfer() selector
        logs: Vec::new(),
        raw_json: "{\"raw\":\"fixture\"}".to_string(),
    }
}

#[tokio::test]
async fn returns_a_successful_tx() {
    let reader = StubTxReaderPort::new();
    let tx = base_tx("0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b");
    reader.insert(tx.clone());

    let got = load_tx_overview::run(&reader, tx.hash, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got.tx, tx);
    assert!(got.decoded_method.is_none());
    assert!(got.decoded_logs.is_empty());
    assert_eq!(
        got.tx.fee_paid().expect("mined tx has fee").value(),
        14_000_000_000u128 * 52_341u128
    );
}

#[tokio::test]
async fn carries_revert_reason_on_failure() {
    let reader = StubTxReaderPort::new();
    let mut tx = base_tx("0xfefe016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944f");
    tx.status = TxStatus::Failed {
        reason: Some("InsufficientBalance()".into()),
    };
    reader.insert(tx.clone());

    let got = load_tx_overview::run(&reader, tx.hash, Chain::Ethereum)
        .await
        .expect("ok");

    match got.tx.status {
        TxStatus::Failed { reason } => {
            assert_eq!(reason.as_deref(), Some("InsufficientBalance()"));
        }
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[tokio::test]
async fn pending_tx_has_no_block_and_pending_status() {
    let reader = StubTxReaderPort::new();
    let mut tx = base_tx("0xbeef016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944a");
    tx.status = TxStatus::Pending;
    tx.block_number = None;
    tx.block_hash = None;
    tx.tx_index = None;
    tx.gas_used = None;
    reader.insert(tx.clone());

    let got = load_tx_overview::run(&reader, tx.hash, Chain::Ethereum)
        .await
        .expect("ok");

    assert!(got.tx.is_pending());
    assert!(got.tx.fee_paid().is_none());
    assert!(got.tx.block_number.is_none());
}

#[tokio::test]
async fn decoding_prefers_abi_over_signature_directory() {
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();
    let detector = StubProxyDetectionPort::new();

    let tx = base_tx("0xaaaa016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394aa");
    reader.insert(tx.clone());

    let to = tx.to.expect("base tx targets a contract");
    let abi = ContractAbi {
        abi: r#"[{"type":"function","name":"transfer","inputs":[{"name":"to","type":"address"},{"name":"value","type":"uint256"}],"outputs":[]}]"#
            .to_string(),
        is_verified: true,
    };
    contract_source.insert(to, abi);
    signatures.set_selector([0xa9, 0x05, 0x9c, 0xbb], "sigdb_override(address,uint256)");

    let got = load_tx_overview::run_with_decoding(
        &reader,
        &contract_source,
        &signatures,
        &detector,
        tx.hash,
        Chain::Ethereum,
    )
    .await
    .expect("ok");

    let method = got.decoded_method.expect("method decoded");
    assert_eq!(method.signature, "transfer(address,uint256)");
    assert_eq!(method.source, SignatureSource::Abi);
}

#[tokio::test]
async fn decoding_falls_back_to_signature_directory() {
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();
    let detector = StubProxyDetectionPort::new();

    let tx = base_tx("0xbbbb016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394bb");
    reader.insert(tx.clone());
    signatures.set_selector([0xa9, 0x05, 0x9c, 0xbb], "transfer(address,uint256)");

    let got = load_tx_overview::run_with_decoding(
        &reader,
        &contract_source,
        &signatures,
        &detector,
        tx.hash,
        Chain::Ethereum,
    )
    .await
    .expect("ok");

    let method = got.decoded_method.expect("method decoded");
    assert_eq!(method.signature, "transfer(address,uint256)");
    assert_eq!(method.source, SignatureSource::SignatureDirectory);
}

#[tokio::test]
async fn logs_are_decoded_via_signature_directory() {
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();
    let detector = StubProxyDetectionPort::new();

    let mut tx = base_tx("0xcccc016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394cc");
    let topic: [u8; 32] =
        hex::decode("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
            .unwrap()
            .try_into()
            .unwrap();
    tx.logs.push(LogEntry {
        address: Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        topics: vec![topic],
        data: Vec::new(),
    });
    reader.insert(tx.clone());
    signatures.set_event_topic(topic, "Transfer(address,address,uint256)");

    let got = load_tx_overview::run_with_decoding(
        &reader,
        &contract_source,
        &signatures,
        &detector,
        tx.hash,
        Chain::Ethereum,
    )
    .await
    .expect("ok");

    assert_eq!(got.decoded_logs.len(), 1);
    let sig = got.decoded_logs[0].signature.as_ref().expect("decoded");
    assert_eq!(sig.signature, "Transfer(address,address,uint256)");
}

#[tokio::test]
async fn it_decodes_method_via_proxy_implementation_abi() {
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();
    let detector = StubProxyDetectionPort::new();

    let tx = base_tx("0xd101016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394d1");
    reader.insert(tx.clone());

    let proxy = tx.to.expect("tx targets proxy");
    let implementation = Address::from_hex("0x1111222233334444555566667777888899990000").unwrap();

    // Proxy ABI is present but lacks `transfer`.
    let proxy_abi = ContractAbi {
        abi: r#"[{"type":"function","name":"implementation","inputs":[],"outputs":[{"name":"","type":"address"}]}]"#
            .to_string(),
        is_verified: true,
    };
    contract_source.insert(proxy, proxy_abi);

    // Detector resolves proxy -> implementation.
    detector.set(
        proxy,
        ProxyInfo {
            kind: ProxyKind::Eip1967,
            implementation,
        },
    );

    // Implementation ABI carries the selector.
    let impl_abi = ContractAbi {
        abi: r#"[{"type":"function","name":"transfer","inputs":[{"name":"to","type":"address"},{"name":"value","type":"uint256"}],"outputs":[{"name":"","type":"bool"}]}]"#
            .to_string(),
        is_verified: true,
    };
    contract_source.insert(implementation, impl_abi);

    let got = load_tx_overview::run_with_decoding(
        &reader,
        &contract_source,
        &signatures,
        &detector,
        tx.hash,
        Chain::Ethereum,
    )
    .await
    .expect("ok");

    let method = got.decoded_method.expect("decoded via proxy impl");
    assert_eq!(method.signature, "transfer(address,uint256)");
    match method.source {
        SignatureSource::ProxyAbi {
            proxy: got_proxy,
            implementation: got_impl,
        } => {
            assert_eq!(got_proxy, proxy);
            assert_eq!(got_impl, implementation);
        }
        other => panic!("expected ProxyAbi, got {other:?}"),
    }
}

#[tokio::test]
async fn it_decodes_method_via_openchain_when_abi_has_no_match() {
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();
    let detector = StubProxyDetectionPort::new();

    let tx = base_tx("0xd202016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394d2");
    reader.insert(tx.clone());

    let proxy = tx.to.expect("tx targets proxy");
    let implementation = Address::from_hex("0x1111222233334444555566667777888899990000").unwrap();

    // Both direct and implementation ABI miss the selector.
    let unrelated_abi = ContractAbi {
        abi: r#"[{"type":"function","name":"foo","inputs":[],"outputs":[]}]"#.to_string(),
        is_verified: true,
    };
    contract_source.insert(proxy, unrelated_abi.clone());
    contract_source.insert(implementation, unrelated_abi);
    detector.set(
        proxy,
        ProxyInfo {
            kind: ProxyKind::Eip1967,
            implementation,
        },
    );

    // Signature directory returns the name.
    signatures.set_selector([0xa9, 0x05, 0x9c, 0xbb], "transfer(address,uint256)");

    let got = load_tx_overview::run_with_decoding(
        &reader,
        &contract_source,
        &signatures,
        &detector,
        tx.hash,
        Chain::Ethereum,
    )
    .await
    .expect("ok");

    let method = got.decoded_method.expect("decoded via signature directory");
    assert_eq!(method.signature, "transfer(address,uint256)");
    assert_eq!(method.source, SignatureSource::SignatureDirectory);
}

#[tokio::test]
async fn asset_changes_become_loaded_when_sim_returns_entries() {
    let sim = StubTxSimulationPort::new();
    let tx = base_tx("0xdada016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394da");
    sim.set_changes(
        tx.hash,
        vec![AssetChange {
            kind: AssetChangeKind::Transfer,
            asset: AssetKind::Native,
            from: Some(tx.from),
            to: tx.to,
            amount: Wei::new(42),
        }],
    );

    let mut view = TxView::bare(tx);
    load_tx_overview::load_asset_changes(&sim, &mut view, Chain::Ethereum).await;
    match view.asset_changes {
        LoadStatus::Loaded(changes) => {
            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].amount.value(), 42);
        }
        other => panic!("expected Loaded, got {other:?}"),
    }
}

#[tokio::test]
async fn asset_changes_become_unsupported_on_feature_unavailable() {
    let sim = StubTxSimulationPort::new();
    sim.mark_unsupported();

    let tx = base_tx("0xabab016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394ab");
    let mut view = TxView::bare(tx);
    load_tx_overview::load_asset_changes(&sim, &mut view, Chain::Ethereum).await;
    assert!(matches!(view.asset_changes, LoadStatus::Unsupported));
}

#[tokio::test]
async fn state_diff_becomes_loaded_when_tracer_returns_entries() {
    let tracer = StubTxTracePort::new();
    let tx = base_tx("0xecec016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394ec");
    tracer.set_state_diff(
        tx.hash,
        StateDiff {
            entries: vec![AddressStateDiff {
                address: tx.from,
                balance: DiffChange::Changed {
                    from: "0xff".into(),
                    to: "0xfe".into(),
                },
                nonce: DiffChange::Unchanged,
                code: DiffChange::Unchanged,
                storage: Vec::new(),
            }],
        },
    );
    let mut view = TxView::bare(tx);
    load_tx_overview::load_state_diff(&tracer, &mut view, Chain::Ethereum).await;
    match view.state_diff {
        LoadStatus::Loaded(diff) => {
            assert_eq!(diff.entries.len(), 1);
            assert!(diff.entries[0].balance.is_change());
        }
        other => panic!("expected Loaded, got {other:?}"),
    }
}

#[tokio::test]
async fn state_diff_becomes_unsupported_on_feature_unavailable() {
    let tracer = StubTxTracePort::new();
    tracer.mark_unsupported();
    let tx = base_tx("0xfbfb016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394fb");
    let mut view = TxView::bare(tx);
    load_tx_overview::load_state_diff(&tracer, &mut view, Chain::Ethereum).await;
    assert!(matches!(view.state_diff, LoadStatus::Unsupported));
}

#[tokio::test]
async fn missing_tx_returns_not_found() {
    let reader = StubTxReaderPort::new();
    let hash =
        TxHash::from_hex("0x0000000000000000000000000000000000000000000000000000000000000001")
            .unwrap();

    let err = load_tx_overview::run(&reader, hash, Chain::Ethereum)
        .await
        .expect_err("missing tx must error");

    assert!(matches!(err, DomainError::NotFound));
}
