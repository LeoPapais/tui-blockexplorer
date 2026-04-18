//! Probe: runs the same use-cases the TUI screens dispatch, against
//! live Alchemy / Etherscan / Sourcify on Polygon mainnet, for a
//! fixed set of fixtures supplied by the user. Read-only; prints a
//! human-readable report.
//!
//! Usage:
//!   ALCHEMY_API_KEY=... ETHERSCAN_API_KEY=... cargo run --example probe_polygon

use std::time::Duration;

use reqwest::Client;
use url::Url;

use blockexplorer_tui::{
    adapters::{
        etherscan::{EtherscanClient, EtherscanContractSource},
        rpc::{
            AlchemyAddressLookup, AlchemyAddressReader, AlchemyBlockLookup, AlchemyBlockReader,
            AlchemyEnsResolver, AlchemyProxyDetector, AlchemyTokenReader, AlchemyTxLookup,
            AlchemyTxReader, RpcClient,
        },
        signatures::SourcifySignatureDirectory,
    },
    application::{
        ports::{AddressLookupPort, ContractSourcePort, TokenSearchPort},
        use_cases::{
            load_address_overview, load_block_overview, load_contract_overview,
            load_token_overview, load_tx_overview, resolve_query::ResolveQuery,
        },
    },
    domain::{
        Address, AddressKind, BlockId, BlockNumber, Chain, ContractAbi, ContractSource,
        DomainError, ResolvedEntity, TokenMetadata, TxHash,
    },
};

/// Null token search. The probe does not care about ticker lookup,
/// and it lets us reuse ResolveQuery without wiring an Etherscan
/// search adapter.
struct NoopTokenSearch;

impl TokenSearchPort for NoopTokenSearch {
    async fn by_symbol(
        &self,
        _symbol: &str,
        _chain: Chain,
    ) -> Result<Vec<TokenMetadata>, DomainError> {
        Ok(Vec::new())
    }
    async fn by_name(&self, _text: &str, _chain: Chain) -> Result<Vec<TokenMetadata>, DomainError> {
        Ok(Vec::new())
    }
}

fn section(title: &str) {
    println!("\n============================================================");
    println!("== {title}");
    println!("============================================================");
}

fn fmt_wei_matic(w: blockexplorer_tui::domain::Wei) -> String {
    let raw = w.value();
    let ether = (raw as f64) / 1e18;
    format!("{raw} wei  (~{ether:.6} MATIC)")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let alchemy_key =
        std::env::var("ALCHEMY_API_KEY").map_err(|_| anyhow::anyhow!("ALCHEMY_API_KEY not set"))?;
    let etherscan_key = std::env::var("ETHERSCAN_API_KEY").ok();

    let chain = Chain::Polygon;
    let url = Url::parse(&format!(
        "https://{sub}.g.alchemy.com/v2/{alchemy_key}",
        sub = chain.alchemy_subdomain(),
    ))?;
    let http = Client::builder().timeout(Duration::from_secs(20)).build()?;
    let rpc = RpcClient::new(url, http);

    let etherscan = match etherscan_key.as_ref() {
        Some(k) => Some(EtherscanContractSource::new(
            EtherscanClient::with_default_http(k.clone())?,
        )),
        None => {
            println!("(warning) ETHERSCAN_API_KEY not set — ABI/source skipped");
            None
        }
    };
    let sigs = SourcifySignatureDirectory::with_default_http()?;

    // Inputs supplied by the operator.
    let token_addr = Address::from_hex("0xe6a537a407488807f0bbeb0038b79004f19dddfb")?;
    let tx_hash =
        TxHash::from_hex("0x7eb65d2c123e35a37cc21048e6aa681b35f2191ce977ae7bf2feeeca0c467e8c")?;
    let eoa_addr = Address::from_hex("0x5abc0e99dfc7ba2c9da42f8dc91ec4128a89e919")?;
    let contract_addr = Address::from_hex("0xab4b63bd6c214ce8409fa1b31afa50d4e17597f9")?;
    let block_no = BlockNumber::new(85_696_170);

    // ---------------------------------------------------------------
    // 1. Block
    // ---------------------------------------------------------------
    section(&format!("Block {}", block_no.value()));
    let block_reader = AlchemyBlockReader::new(rpc.clone());
    match load_block_overview::run(&block_reader, BlockId::Number(block_no), chain).await {
        Ok(b) => {
            println!("number     : {}", b.number.value());
            println!("hash       : {}", b.hash.to_hex());
            println!("parent     : {}", b.parent_hash.to_hex());
            println!("timestamp  : {}", b.timestamp.seconds());
            println!("miner      : {}", b.miner.to_hex());
            println!("gas used   : {}", b.gas_used);
            println!("gas limit  : {}", b.gas_limit);
            println!("base fee   : {:?}", b.base_fee.map(|w| w.value()));
            println!("size       : {}", b.size);
            println!("tx count   : {}", b.tx_hashes.len());
            if !b.tx_hashes.is_empty() {
                println!("first tx   : {}", b.tx_hashes[0].to_hex());
            }
        }
        Err(e) => println!("ERROR: {e}"),
    }

    // ---------------------------------------------------------------
    // 2. EOA
    // ---------------------------------------------------------------
    section(&format!("EOA {}", eoa_addr.to_hex()));
    let addr_reader = AlchemyAddressReader::new(rpc.clone());
    let addr_lookup = AlchemyAddressLookup::new(rpc.clone());
    match addr_lookup.classify(eoa_addr, chain).await {
        Ok(AddressKind::Eoa) => println!("classify   : Eoa  [OK, expected EOA]"),
        Ok(AddressKind::Contract) => {
            println!("classify   : Contract  [!! unexpected — user said this was an EOA]");
        }
        Err(e) => println!("classify ERROR: {e}"),
    }
    // Raw eth_getCode probe so we can see whether this is empty
    // bytecode (pure EOA), a 7702 delegation tag (0xef0100...), or
    // real contract code.
    {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getCode",
            "params": [eoa_addr.to_hex(), "latest"],
        });
        let resp: serde_json::Value = reqwest::Client::new()
            .post(Url::parse(&format!(
                "https://{sub}.g.alchemy.com/v2/{alchemy_key}",
                sub = chain.alchemy_subdomain(),
            ))?)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;
        let code = resp
            .get("result")
            .and_then(|v| v.as_str())
            .unwrap_or("0x")
            .to_string();
        let body_no_pref = code.trim_start_matches("0x");
        let n_bytes = body_no_pref.len() / 2;
        let is_7702 = body_no_pref.to_lowercase().starts_with("ef0100");
        println!(
            "getCode    : {n_bytes} bytes  (prefix {}{})",
            body_no_pref.chars().take(12).collect::<String>(),
            if is_7702 {
                "  -> EIP-7702 delegator"
            } else {
                ""
            }
        );
    }
    match load_address_overview::run(&addr_reader, eoa_addr, chain).await {
        Ok(a) => {
            println!("balance    : {}", fmt_wei_matic(a.balance));
            println!("nonce      : {}", a.nonce);
            println!("kind       : {:?}", a.kind);
        }
        Err(e) => println!("overview ERROR: {e}"),
    }

    // ---------------------------------------------------------------
    // 3. Contract (non-ERC20)
    // ---------------------------------------------------------------
    section(&format!("Contract {}", contract_addr.to_hex()));
    let proxy = AlchemyProxyDetector::new(rpc.clone());
    match addr_lookup.classify(contract_addr, chain).await {
        Ok(AddressKind::Contract) => println!("classify   : Contract  [OK]"),
        Ok(AddressKind::Eoa) => {
            println!("classify   : Eoa  [!! user said this was a contract]");
        }
        Err(e) => println!("classify ERROR: {e}"),
    }
    match load_contract_overview::run(&addr_reader, &proxy, contract_addr, chain).await {
        Ok(co) => {
            println!("balance    : {}", fmt_wei_matic(co.account.balance));
            println!("nonce      : {}", co.account.nonce);
            println!("kind       : {:?}", co.account.kind);
            match co.proxy {
                Some(p) => println!(
                    "proxy      : kind={:?} impl={}",
                    p.kind,
                    p.implementation.to_hex()
                ),
                None => println!("proxy      : (not a proxy)"),
            }
        }
        Err(e) => println!("overview ERROR: {e}"),
    }
    if let Some(es) = etherscan.as_ref() {
        match es.get_abi(contract_addr, chain).await {
            Ok(Some(ContractAbi { abi, is_verified })) => {
                let fn_count = abi.matches("\"type\":\"function\"").count();
                println!(
                    "abi        : verified={is_verified} functions={fn_count}  ({} bytes)",
                    abi.len()
                );
            }
            Ok(None) => println!("abi        : (not available)"),
            Err(e) => println!("abi ERROR  : {e}"),
        }
        match es.get_source(contract_addr, chain).await {
            Ok(Some(ContractSource {
                is_verified,
                contract_name,
                compiler_version,
                license,
                files,
                ..
            })) => {
                println!(
                    "source     : name={contract_name}  solc={compiler_version}  \
                     license={license}  verified={is_verified}  files={}",
                    files.len()
                );
            }
            Ok(None) => println!("source     : (not verified)"),
            Err(e) => println!("source ERROR: {e}"),
        }
    }

    // ---------------------------------------------------------------
    // 4. Token (ERC-20)
    // ---------------------------------------------------------------
    section(&format!("Token ERC-20 {}", token_addr.to_hex()));
    let token_reader = AlchemyTokenReader::new(rpc.clone());
    match load_token_overview::run(&token_reader, token_addr, chain).await {
        Ok(t) => {
            println!(
                "metadata   : symbol={}  name={}  decimals={}",
                t.metadata.symbol, t.metadata.name, t.metadata.decimals
            );
            let scaled = (t.total_supply as f64) / 10f64.powi(i32::from(t.metadata.decimals));
            println!("totalSupply: {} raw  (~{scaled:.4})", t.total_supply);
            match t.price {
                Some(p) => println!("price      : ${:.6} (currency {})", p.value, p.currency),
                None => println!("price      : (none)"),
            }
        }
        Err(e) => println!("overview ERROR: {e}"),
    }

    // ---------------------------------------------------------------
    // 5. Transaction
    // ---------------------------------------------------------------
    section(&format!("Tx {}", tx_hash.to_hex()));
    let tx_reader = AlchemyTxReader::new(rpc.clone());
    let tx_proxy_detector = AlchemyProxyDetector::new(rpc.clone());
    let view = if let Some(es) = etherscan.as_ref() {
        load_tx_overview::run_with_decoding(
            &tx_reader,
            es,
            &sigs,
            &tx_proxy_detector,
            tx_hash,
            chain,
        )
        .await
    } else {
        load_tx_overview::run_with_decoding(
            &tx_reader,
            &NoopContractSource,
            &sigs,
            &tx_proxy_detector,
            tx_hash,
            chain,
        )
        .await
    };
    match view {
        Ok(v) => {
            let t = &v.tx;
            println!("hash       : {}", t.hash.to_hex());
            println!("status     : {:?}", t.status);
            println!(
                "block      : {:?} idx {:?}",
                t.block_number.map(|n| n.value()),
                t.tx_index
            );
            println!("from       : {}", t.from.to_hex());
            println!(
                "to         : {}",
                t.to.as_ref()
                    .map(|a| a.to_hex())
                    .unwrap_or_else(|| "(contract creation)".into())
            );
            println!("value      : {}", fmt_wei_matic(t.value));
            println!("gas used   : {:?}", t.gas_used);
            println!("gas limit  : {}", t.gas_limit);
            println!("gas price  : {} wei", t.gas_price.value());
            println!("nonce      : {}", t.nonce);
            println!("type       : {:?}", t.tx_type);
            println!("input bytes: {}", t.input.len());
            if t.input.len() >= 4 {
                let sel = &t.input[..4];
                println!(
                    "selector   : 0x{:02x}{:02x}{:02x}{:02x}",
                    sel[0], sel[1], sel[2], sel[3]
                );
            }
            println!("logs       : {}", t.logs.len());
            match v.decoded_method {
                Some(dm) => println!("method     : {}  [source={:?}]", dm.signature, dm.source),
                None => println!("method     : (not decoded)"),
            }
            let decoded_logs = v
                .decoded_logs
                .iter()
                .filter(|l| l.signature.is_some())
                .count();
            println!(
                "logs decod.: {} / {} via signature lookup",
                decoded_logs,
                v.decoded_logs.len()
            );
        }
        Err(e) => println!("tx ERROR  : {e}"),
    }

    // ---------------------------------------------------------------
    // 6. Universal search — reproduces the Search screen's
    //    ResolveQuery on each fixture. Uses real Alchemy lookups so
    //    we validate classify() + routing end-to-end.
    // ---------------------------------------------------------------
    section("ResolveQuery (universal search)");
    let block_lookup = AlchemyBlockLookup::new(rpc.clone());
    let tx_lookup = AlchemyTxLookup::new(rpc.clone());
    let ens = AlchemyEnsResolver::new(rpc.clone());
    let token_search = NoopTokenSearch;
    let resolver = ResolveQuery {
        block: &block_lookup,
        tx: &tx_lookup,
        address: &addr_lookup,
        ens: &ens,
        token: &token_search,
    };
    for (label, input) in [
        ("block-number", "85696170"),
        ("tx-hash", &tx_hash.to_hex()),
        ("token-address", &token_addr.to_hex()),
        ("contract-address", &contract_addr.to_hex()),
        ("eoa-address", &eoa_addr.to_hex()),
    ] {
        match resolver.run(input, chain).await {
            Ok(rs) => {
                println!("-- {label} ({input}) --");
                for r in rs {
                    match r {
                        ResolvedEntity::Block { number, hash } => println!(
                            "   Block      number={} hash={}",
                            number.value(),
                            hash.to_hex()
                        ),
                        ResolvedEntity::Tx { hash, block } => println!(
                            "   Tx         hash={} block={:?}",
                            hash.to_hex(),
                            block.map(|b| b.value())
                        ),
                        ResolvedEntity::Address {
                            address,
                            kind,
                            ens_name,
                        } => println!(
                            "   Address    {} kind={:?} ens={:?}",
                            address.to_hex(),
                            kind,
                            ens_name
                        ),
                        ResolvedEntity::Contract { address } => {
                            println!("   Contract   {}", address.to_hex())
                        }
                        ResolvedEntity::Token(m) => println!(
                            "   Token      {} {}/{} dec={}",
                            m.address.to_hex(),
                            m.symbol,
                            m.name,
                            m.decimals
                        ),
                        ResolvedEntity::NotFound { reason } => {
                            println!("   NotFound   {}", reason)
                        }
                    }
                }
            }
            Err(e) => println!("-- {label} ({input}) -- ERROR: {e}"),
        }
    }

    println!("\nprobe done.");
    Ok(())
}

/// Local fallback when no Etherscan key is configured. The real
/// infra wires `TxContractSource::Noop`; we mirror it here so we
/// can reach `load_tx_overview::run_with_decoding`.
#[derive(Clone, Copy)]
struct NoopContractSource;

impl ContractSourcePort for NoopContractSource {
    async fn get_abi(
        &self,
        _address: Address,
        _chain: Chain,
    ) -> Result<Option<ContractAbi>, DomainError> {
        Ok(None)
    }
    async fn get_source(
        &self,
        _address: Address,
        _chain: Chain,
    ) -> Result<Option<ContractSource>, DomainError> {
        Ok(None)
    }
}
