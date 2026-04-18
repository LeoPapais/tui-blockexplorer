//! Functional tests for the `load_tx_overview` use case.
//!
//! See `plan/4-tx-detail.md` section 12.1.

use blockexplorer_tui::{
    application::{LoadStatus, SignatureSource, TxView, use_cases::load_tx_overview},
    domain::{
        Address, AddressStateDiff, AssetChange, AssetChangeKind, AssetKind, BlockHash, BlockNumber,
        CallKind, CallNode, Chain, ContractAbi, DiffChange, DomainError, LogEntry, ProxyInfo,
        StateDiff, Transaction, TxHash, TxStatus, TxType, Wei,
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
    assert_eq!(method.source, SignatureSource::Openchain);
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
    detector.set(proxy, ProxyInfo::eip1967_slot(implementation));

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
    detector.set(proxy, ProxyInfo::eip1967_slot(implementation));

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
    assert_eq!(method.source, SignatureSource::Openchain);
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

// Plan 12.6.5 — `load_call_tree` populates the Internal tab status.
#[tokio::test]
async fn call_tree_becomes_loaded_when_tracer_returns_a_tree() {
    let tracer = StubTxTracePort::new();
    let tx = base_tx("0xca11016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394ca");
    let child_to = Address::from_hex("0x1111111111111111111111111111111111111111").unwrap();
    let tree = CallNode {
        kind: CallKind::Call,
        from: tx.from,
        to: tx.to,
        value: Wei::new(0),
        input: Vec::new(),
        output: Vec::new(),
        gas_used: 52_341,
        error: None,
        children: vec![CallNode {
            kind: CallKind::Staticcall,
            from: tx.to.unwrap(),
            to: Some(child_to),
            value: Wei::new(0),
            input: Vec::new(),
            output: Vec::new(),
            gas_used: 128,
            error: None,
            children: Vec::new(),
        }],
    };
    tracer.set_call_tree(tx.hash, tree);
    let mut view = TxView::bare(tx);
    load_tx_overview::load_call_tree(&tracer, &mut view, Chain::Ethereum).await;
    match view.call_tree {
        LoadStatus::Loaded(root) => {
            assert_eq!(root.frame_count(), 2);
            assert_eq!(root.kind, CallKind::Call);
            assert_eq!(root.children[0].kind, CallKind::Staticcall);
        }
        other => panic!("expected Loaded, got {other:?}"),
    }
    assert_eq!(tracer.call_tree_count(), 1);
}

#[tokio::test]
async fn call_tree_becomes_unsupported_on_feature_unavailable() {
    let tracer = StubTxTracePort::new();
    tracer.mark_unsupported();
    let tx = base_tx("0xdedf016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394de");
    let mut view = TxView::bare(tx);
    load_tx_overview::load_call_tree(&tracer, &mut view, Chain::Ethereum).await;
    assert!(matches!(view.call_tree, LoadStatus::Unsupported));
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

// ---------------------------------------------------------------------------
// Plan 12.6.4 — ABI-driven argument decoding for logs
// ---------------------------------------------------------------------------
//
// The three tests below exercise the real ABI indexed / non-indexed
// split instead of the "first N positional args are indexed"
// heuristic used for signature-directory hits.

fn log_with_topic_and_data(
    address: Address,
    topic0: [u8; 32],
    extra_topics: Vec<[u8; 32]>,
    data: Vec<u8>,
) -> LogEntry {
    let mut topics = vec![topic0];
    topics.extend(extra_topics);
    LogEntry {
        address,
        topics,
        data,
    }
}

fn word_with_u128(n: u128) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[16..].copy_from_slice(&n.to_be_bytes());
    w
}

fn word_with_address(hex_addr: &str) -> [u8; 32] {
    let mut w = [0u8; 32];
    let bytes = hex::decode(hex_addr.trim_start_matches("0x")).unwrap();
    w[12..].copy_from_slice(&bytes);
    w
}

#[tokio::test]
async fn logs_abi_decoding_respects_indexed_flags_for_erc20() {
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();
    let detector = StubProxyDetectionPort::new();

    let mut tx = base_tx("0xabc0016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394ab");
    let topic0: [u8; 32] =
        hex::decode("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
            .unwrap()
            .try_into()
            .unwrap();
    let from_topic = word_with_address("d8da6bf26964af9d7eed9e03e53415d37aa96045");
    let to_topic = word_with_address("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let value_word = word_with_u128(1_000);
    let contract = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    tx.logs.push(log_with_topic_and_data(
        contract,
        topic0,
        vec![from_topic, to_topic],
        value_word.to_vec(),
    ));
    reader.insert(tx.clone());
    // ERC20 Transfer: indexed from, indexed to, value (non-indexed).
    let abi = blockexplorer_tui::domain::ContractAbi {
        abi: r#"[{"type":"event","name":"Transfer","inputs":[
            {"name":"from","type":"address","indexed":true},
            {"name":"to","type":"address","indexed":true},
            {"name":"value","type":"uint256","indexed":false}
        ],"anonymous":false}]"#
            .to_string(),
        is_verified: true,
    };
    contract_source.insert(contract, abi);

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
    let parsed = sig.parsed.as_ref().expect("parsed ABI present");
    assert_eq!(parsed.params.len(), 3);
    assert_eq!(parsed.params[0].name, "from");
    assert!(parsed.params[0].indexed);
    assert_eq!(parsed.params[1].name, "to");
    assert!(parsed.params[1].indexed);
    assert_eq!(parsed.params[2].name, "value");
    assert!(!parsed.params[2].indexed);
}

#[tokio::test]
async fn logs_abi_decoding_handles_erc721_all_indexed() {
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();
    let detector = StubProxyDetectionPort::new();

    let mut tx = base_tx("0xaba7016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394a7");
    let topic0: [u8; 32] =
        hex::decode("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
            .unwrap()
            .try_into()
            .unwrap();
    let from_topic = word_with_address("d8da6bf26964af9d7eed9e03e53415d37aa96045");
    let to_topic = word_with_address("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let token_id = word_with_u128(42);
    let contract = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    tx.logs.push(log_with_topic_and_data(
        contract,
        topic0,
        vec![from_topic, to_topic, token_id],
        Vec::new(),
    ));
    reader.insert(tx.clone());
    // ERC721 Transfer: all three args are indexed.
    let abi = blockexplorer_tui::domain::ContractAbi {
        abi: r#"[{"type":"event","name":"Transfer","inputs":[
            {"name":"from","type":"address","indexed":true},
            {"name":"to","type":"address","indexed":true},
            {"name":"tokenId","type":"uint256","indexed":true}
        ],"anonymous":false}]"#
            .to_string(),
        is_verified: true,
    };
    contract_source.insert(contract, abi);

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

    let sig = got.decoded_logs[0].signature.as_ref().expect("decoded");
    let parsed = sig.parsed.as_ref().expect("parsed ABI present");
    assert_eq!(parsed.params.len(), 3);
    assert!(parsed.params.iter().all(|p| p.indexed));
    assert_eq!(parsed.params[2].name, "tokenId");
}

#[tokio::test]
async fn logs_abi_decoding_handles_custom_mixed_event() {
    // Custom event: non-leading indexed arg.
    // `Ping(uint256 id, string note, address indexed who)`
    // Signature selector (for the test to drive the stub): compute
    // via the same helper the use case uses.
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();
    let detector = StubProxyDetectionPort::new();

    let mut tx = base_tx("0xabc2016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394ab");
    let signature_text = "Ping(uint256,string,address)";
    let topic0 = blockexplorer_tui::domain::contract_source::event_topic_for(signature_text);
    let who = word_with_address("d8da6bf26964af9d7eed9e03e53415d37aa96045");
    // Non-indexed: id (uint256) then string. For simplicity the
    // string body is dropped onto an arbitrary 32-byte word; the
    // decoder will fall back to raw-hex for the string type, which
    // is fine — the test focuses on alignment.
    let id_word = word_with_u128(7);
    let mut data = Vec::new();
    data.extend_from_slice(&id_word); // id
    data.extend_from_slice(&id_word); // string offset placeholder (not decoded)
    let contract = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    tx.logs.push(log_with_topic_and_data(
        contract,
        topic0,
        vec![who], // single indexed arg at topic[1]
        data,
    ));
    reader.insert(tx.clone());
    let abi = blockexplorer_tui::domain::ContractAbi {
        abi: r#"[{"type":"event","name":"Ping","inputs":[
            {"name":"id","type":"uint256","indexed":false},
            {"name":"note","type":"string","indexed":false},
            {"name":"who","type":"address","indexed":true}
        ],"anonymous":false}]"#
            .to_string(),
        is_verified: true,
    };
    contract_source.insert(contract, abi);

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

    let sig = got.decoded_logs[0].signature.as_ref().expect("decoded");
    let parsed = sig.parsed.as_ref().expect("parsed ABI present");
    assert_eq!(parsed.params.len(), 3);
    // `who` is the third positional arg but the only indexed one.
    // A naive "first N are indexed" heuristic would get this wrong;
    // the ABI-driven decoder must honour the `indexed` flag.
    assert_eq!(parsed.params[0].indexed, false);
    assert_eq!(parsed.params[1].indexed, false);
    assert_eq!(parsed.params[2].indexed, true);
    assert_eq!(parsed.params[2].name, "who");
    assert_eq!(parsed.params[2].type_, "address");
}

#[tokio::test]
async fn logs_directory_hit_leaves_parsed_none() {
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();
    let detector = StubProxyDetectionPort::new();

    let mut tx = base_tx("0xabcf016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394ac");
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

    let sig = got.decoded_logs[0].signature.as_ref().expect("decoded");
    assert_eq!(sig.signature, "Transfer(address,address,uint256)");
    assert!(
        sig.parsed.is_none(),
        "directory hits cannot report indexed flags"
    );
}

// ---------------------------------------------------------------------------
// Plan 12.6.1 — Overview tab must never wait on slow RPC methods.
// ---------------------------------------------------------------------------

/// `run_with_decoding` must not call the tracer or the simulator.
/// Those live on `load_state_diff` / `load_asset_changes` so the
/// Overview tab can render off the reader + Etherscan response
/// alone, without waiting for the slow trace/debug methods.
#[tokio::test]
async fn run_with_decoding_never_hits_tracer_or_simulator() {
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();
    let detector = StubProxyDetectionPort::new();
    let sim = StubTxSimulationPort::new();
    let tracer = StubTxTracePort::new();

    let tx = base_tx("0xfa51016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394fa");
    reader.insert(tx.clone());

    let _ = load_tx_overview::run_with_decoding(
        &reader,
        &contract_source,
        &signatures,
        &detector,
        tx.hash,
        Chain::Ethereum,
    )
    .await
    .expect("decoding ok");

    assert_eq!(sim.call_count(), 0, "simulator must not be touched");
    assert_eq!(tracer.call_count(), 0, "tracer must not be touched");
}

/// When the simulator / tracer are slow, the base overview view
/// must still be delivered first. This mirrors what
/// `infra::tx_feed::spawn_full` does: it sends the base view before
/// awaiting the enrichment join. We exercise the two steps here
/// directly against the use case.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn base_view_resolves_before_tracer_finishes() {
    let reader = StubTxReaderPort::new();
    let contract_source = StubContractSourcePort::new();
    let signatures = StubSignatureDirectoryPort::new();
    let detector = StubProxyDetectionPort::new();
    let sim = StubTxSimulationPort::new();
    let tracer = StubTxTracePort::new();

    let tx = base_tx("0xde1a016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a71394de");
    reader.insert(tx.clone());
    sim.set_delay(std::time::Duration::from_secs(5));
    tracer.set_delay(std::time::Duration::from_secs(5));
    tracer.set_state_diff(
        tx.hash,
        StateDiff {
            entries: Vec::new(),
        },
    );

    // Base view: reader + ABI + signature lookup only. No delay
    // injected by our trace/sim stubs fires yet.
    let base = load_tx_overview::run_with_decoding(
        &reader,
        &contract_source,
        &signatures,
        &detector,
        tx.hash,
        Chain::Ethereum,
    )
    .await
    .expect("ok");
    assert_eq!(sim.call_count(), 0);
    assert_eq!(tracer.call_count(), 0);
    assert!(matches!(
        base.asset_changes,
        blockexplorer_tui::application::LoadStatus::Pending
    ));
    assert!(matches!(
        base.state_diff,
        blockexplorer_tui::application::LoadStatus::Pending
    ));

    // Enrichment path: must call sim + tracer exactly once.
    // Clone into two views so the concurrent enrichments do not
    // alias the same mutable borrow (mirrors how `spawn_full`
    // splits the join body).
    let mut view_sim = base.clone();
    let mut view_trace = base.clone();
    tokio::join!(
        load_tx_overview::load_asset_changes(&sim, &mut view_sim, Chain::Ethereum),
        load_tx_overview::load_state_diff(&tracer, &mut view_trace, Chain::Ethereum),
    );
    assert_eq!(sim.call_count(), 1);
    assert_eq!(tracer.call_count(), 1);
    assert!(!matches!(
        view_sim.asset_changes,
        blockexplorer_tui::application::LoadStatus::Pending
    ));
    assert!(!matches!(
        view_trace.state_diff,
        blockexplorer_tui::application::LoadStatus::Pending
    ));
}
