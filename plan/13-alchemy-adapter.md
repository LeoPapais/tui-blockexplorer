# 13 — Alchemy adapter

Phase 2 of the Option B roadmap. Implements the Alchemy HTTP JSON-RPC
client and wires it into the two outbound ports currently used by the
Home screen: `NetworkStatusPort` and `GasOraclePort`. WebSocket
subscriptions stay out of scope; the runtime polls once per tick which
is enough to deliver a working Home.

## 1. Purpose and scope

- Add a small JSON-RPC client at `src/adapters/rpc/client.rs` that
  speaks to Alchemy's HTTP endpoint. No SDK dependency: reqwest + serde.
- Add `AlchemyNetworkStatusAdapter` implementing `NetworkStatusPort`
  via `eth_blockNumber` + `eth_getBlockByNumber`.
- Add `AlchemyGasOracleAdapter` implementing `GasOraclePort` via
  `eth_gasPrice`, `eth_maxPriorityFeePerGas` and `eth_feeHistory`.
Out of scope for phase 2:

- HomeScreen refresh wiring (phase 3 — requires the config + runtime
  task plumbing done in the same slice).
- Config file parsing and secret loading (phase 3,
  `plan/14-config-and-credentials.md`).
- WebSocket / newHeads subscription.
- Any adapter other than these two ports.
- Live smoke test against Alchemy servers (phase 3, once credentials
  can be loaded).

## 2. JSON-RPC client

Located at `src/adapters/rpc/client.rs`.

### 2.1 Public API

```rust
pub struct RpcClient { /* reqwest::Client + base URL */ }

impl RpcClient {
    pub fn new(base_url: Url, http: reqwest::Client) -> Self;
    pub async fn call<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R, RpcError>;
}
```

### 2.2 Error model

```rust
pub enum RpcError {
    Http(reqwest::Error),
    Rpc { code: i32, message: String },
    Decode(serde_json::Error),
    Rate,          // HTTP 429 or JSON-RPC -32005
    Timeout,
}
```

Mapped into `DomainError` at the adapter boundary:

- `RpcError::Rate` -> `DomainError::ProviderUnavailable` (for now).
- `RpcError::Http` -> `DomainError::ProviderUnavailable`.
- `RpcError::Rpc { .. }` -> `DomainError::Internal(message)`.
- `RpcError::Decode` -> `DomainError::Internal(msg)`.
- `RpcError::Timeout` -> `DomainError::ProviderUnavailable`.

### 2.3 Retry policy

Kept minimal for this phase: one retry on transient errors (429 / 5xx /
reqwest timeout). Full exponential-backoff lands in a future pass.

### 2.4 URL strategy

`Chain::alchemy_subdomain` maps a `Chain` to Alchemy's subdomain
(`eth-mainnet`, `eth-sepolia`, `base-mainnet`, `opt-mainnet`,
`arb-mainnet`, `polygon-mainnet`). The adapter builds
`https://{subdomain}.g.alchemy.com/v2/{api_key}` per request so
multi-chain usage stays data-driven.

## 3. Adapters

### 3.1 `AlchemyNetworkStatusAdapter`

- Dependencies: `RpcClient`, `ApiCredentials { alchemy: String }`,
  `ChainRegistryPort` (for chain metadata).
- `snapshot(chain)` issues `eth_blockNumber` to get the head; then
  `eth_getBlockByNumber(head, false)` to read the base fee and the
  parent timestamp delta (used to estimate `block_time_avg_ms`).
- Returns `NetworkStatus` with `block_time_avg_ms` estimated as the
  difference between the head's timestamp and its parent's timestamp;
  keeps a history window to smooth when repeated calls are made.

### 3.2 `AlchemyGasOracleAdapter`

- `snapshot(chain)` issues three calls in parallel via `tokio::join!`:
  `eth_gasPrice`, `eth_maxPriorityFeePerGas`,
  `eth_feeHistory(20, "latest", [25, 50, 75])`.
- Builds `GasSnapshot`:
  - `base_fee` -> latest entry in `feeHistory.baseFeePerGas`.
  - `slow` / `average` / `fast` -> `base_fee` + the 25 / 50 / 75
    percentile of the reward array.
  - `trend` -> last 20 base fees from `feeHistory`.
- Gracefully degrades when `eth_feeHistory` is unsupported on the
  chain (some L2s): returns `DomainError::FeatureUnavailable`.

## 4. Home screen refresh wiring (deferred to phase 3)

The runtime task that drives `HomeSession::refresh` against the real
adapter and pushes updates into `HomeScreen` is deliberately *not* part
of this phase. Phase 3 (`plan/14-config-and-credentials.md`) introduces
the background feed alongside config and credentials plumbing so the
whole "real data on screen" story lands in one coherent slice.

Until then:

- `cargo run -- --demo` keeps rendering the frozen `HomeViewModel`.
- The Alchemy adapters are reachable from the application layer but no
  user-facing code path invokes them yet. They are only exercised by
  the functional tests described in section 5.

## 5. Tests

### 5.1 Adapter functional tests (new)

- `tests/functional/alchemy_network_status.rs`: wiremock returns a
  canned JSON-RPC response for `eth_blockNumber` and
  `eth_getBlockByNumber`; adapter is called; result asserted.
- `tests/functional/alchemy_gas_oracle.rs`: wiremock returns canned
  responses for the three gas calls; adapter is called; result
  asserted. Plus a failing-path test with a 429 response.

### 5.2 Fixtures

- `tests/fixtures/rpc__eth_blockNumber__ethereum.json`
- `tests/fixtures/rpc__eth_getBlockByNumber__ethereum_head.json`
- `tests/fixtures/rpc__eth_gasPrice__ethereum.json`
- `tests/fixtures/rpc__eth_maxPriorityFeePerGas__ethereum.json`
- `tests/fixtures/rpc__eth_feeHistory__ethereum_20_blocks.json`
- `tests/fixtures/rpc__error__429.json`

### 5.3 Home e2e

Stay untouched. The e2e driver still wires stubs; the feed path is
exercised only by the runtime task.

## 6. Acceptance

- `cargo test` passes. New adapter tests use wiremock only; no live
  network access in CI.
- `cargo clippy --all-targets -- -D warnings` is clean.
- `ALCHEMY_API_KEY=... cargo run` will still not talk to Alchemy yet
  because the key plumbing is phase 3. Manual smoke test of the
  adapter is documented here as "only via wiremock tests until
  phase 3".
- `cargo run -- --demo` keeps rendering the frozen view-model.

## 7. Follow-up (phase 3)

`plan/14-config-and-credentials.md` introduces:

- Config file + env var reader for `ALCHEMY_API_KEY`.
- Replaces `--demo` with automatic selection: use the Alchemy adapter
  when credentials are present, fall back to the demo view model
  otherwise.
- Rudimentary Settings screen to hold the key and the active chain.
