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

## 8. Hardening (plan/15 §8.14)

Six follow-ups from `plan/15-backlog.md` §8.14 tightening the Alchemy
adapter. Organised so the earlier sub-sections are cheap correctness
wins and the later ones structural. Every sub-section ends with a
**Shipped** / **Deferred** marker so promotions stay auditable.

### 8.1 Smarter JSON-RPC error mapping

**Shipped.** The adapter-boundary helper `RpcError::into_domain`
maps `-32602 Invalid params` into `DomainError::InvalidInput` (which
already carries a `String` payload and is the domain newtype for
"caller handed us something the provider rejected"; no extra variant
added to keep `DomainError` lean — see `.cursor/rules/rust-style.mdc`).
`-32601 Method not found` and Alchemy-specific `-32004 Method not
supported` become `DomainError::FeatureUnavailable`; this matches
existing call sites that already treat unsupported methods as a
feature toggle (`AlchemyGasOracleAdapter`, `AlchemyPortfolio`). The
remaining `-32000..=-32099` "server error" range maps to
`ProviderUnavailable` so retries + circuit breaker can act on it the
same way as a 5xx.

Fixtures:
- `rpc__error__invalid_params.json` (`-32602`).
- `rpc__error__method_not_found.json` already existed and is extended
  to cover `-32601` / `-32004`.
- `rpc__error__generic_server.json` (`-32000`).

Functional tests: `tests/functional/rpc_error_mapping.rs` (pure,
drives `RpcError::into_domain` directly so it stays cheap and
readable).

### 8.2 Exponential-backoff retry with jitter

**Shipped.** A new `retry::RetryPolicy` lives in
`src/adapters/rpc/retry.rs` with `max_attempts = 3`,
`base_delay = 200 ms` and a user-supplied `Arc<dyn Rng>` used to
compute jitter. Delay at attempt `n` is
`base * 2^n + rng_draw_in([0, base))`, capped at 5 seconds. The
helper `retry_with_backoff` is operation-shape-agnostic
(`FnMut() -> Fut<Result<T, RpcError>>`) and is wired through
`RpcClient::call` so every adapter inherits retries for free.

Retry classifier: HTTP 429, HTTP 5xx (new `RpcError::HttpServerError`
variant replacing the previous overload of `RpcError::Rate`), JSON-RPC
`-32005 rate limit exceeded`, reqwest timeouts and connect errors.
`RpcError::is_retryable()` is the public predicate so the circuit
breaker (§8.3) can reuse it.

`RpcClient::new(base, http)` stays backwards compatible and ships with
`RetryPolicy::none()`; call sites opt in via
`RpcClient::with_retry_policy` or the convenience
`RpcClient::with_default_retry(rng)`. Infra wires an `Arc<OsRng>` once
in `boot_runtime`.

Tests:
- `tests/functional/rpc_retry.rs` drives `wiremock` with a
  sequential responder (`429 → 500 → 200`) and asserts the final
  call succeeds after two retries, plus a `-32005 → -32005 → -32005`
  scenario that exhausts attempts and surfaces
  `DomainError::ProviderUnavailable`. A unit test pins
  `retry::backoff_for(attempt, base, rng)` so the jitter window is
  enforced and cannot exceed `3 * attempts`.

### 8.3 Per-adapter circuit breaker

**Shipped.** `src/adapters/rpc/circuit_breaker.rs` exposes
`CircuitBreaker::new(failure_threshold, cool_down, clock)` with
`record_success`, `record_failure` and `is_open`. After
`failure_threshold` consecutive failures the breaker opens for
`cool_down`; calls made while open short-circuit with
`DomainError::ProviderUnavailable`. Failures are scoped to
"retryable" (same predicate as §8.2); other errors do not trip the
breaker so a method-not-found does not knock out the whole provider.

`RpcClient::with_circuit_breaker` composes a shared `Arc<CircuitBreaker>`
into the client; the breaker wraps `call` / `call_batch`. State is
`RwLock<FailureState>` on a tiny struct so contention is near-zero.
Unit tests use `FrozenClock` to step past the cool-down and confirm
the breaker transitions `open → half-open → closed`.

`is_open()` stays public so the composite signature directory
pattern (`plan/15-backlog.md` §3.2) can skip a dead provider later
without having to thread another abstraction.

### 8.4 Cost hints (MVP groundwork)

**Shipped.** `src/adapters/rpc/cost_hint.rs` defines
`CostHint { compute_units: u32, kind: CostKind }` and
`rpc::cost_hint_for(method: &str) -> CostHint`. Values come from the
published Alchemy compute-unit table for the MVP methods
(`eth_blockNumber = 10`, `eth_getBlockByNumber = 16`,
`eth_feeHistory = 150`, `eth_call = 26`, `trace_transaction = 309`,
etc.); the fallback is `CostKind::Unknown` with a neutral
`compute_units = 100`.

`src/infra/cost_meter.rs` holds a tiny thread-safe counter
(`AtomicU64`) that `RpcClient::call` bumps on every successful
request. `CostMeter::consumed() -> u64` is exposed but no UI surface
is wired yet — the intention is to hook it into the future CU budget
surface surfaced by `plan/9-gas-tracker.md` without a second pass.
Functional test: `tests/functional/cost_meter.rs` drives three
recorded calls through a stubbed RPC and asserts the meter sums
correctly.

### 8.5 JSON-RPC batch requests

**Shipped.** `RpcClient::call_batch::<P, R>(calls: Vec<BatchCall<P>>)
-> Result<Vec<Result<R, RpcError>>, RpcError>` posts a JSON array
(`[{...}, {...}]`), parses the response array and returns per-call
results. The helper preserves ordering by matching server-side `id`
back to the position in the request (Alchemy returns the array in
request order but the spec does not require it).

`AlchemyPortfolio::get_token_balances` is migrated: the previous
fan-out of `tokio::spawn` per-holding calling `alchemy_getTokenMetadata`
becomes a single batched request. When only one holding is present we
skip the batch wrapper to avoid the array allocation. Test:
`tests/functional/alchemy_portfolio.rs` grows a scenario that
expects a JSON-RPC array body and returns a single response array.

`eth_callMany` is **deferred** — it would replace
`AlchemyContractReader::read_many` (future work); the batch helper
already covers the immediate fan-out win.

### 8.6 newHeads WebSocket adapter

**Shipped.** `src/adapters/rpc/new_heads_stream.rs` hosts
`AlchemyNewHeadsStream`, using the same WebSocket/retry scaffolding as other
Alchemy WS adapters. The task
connects to `wss://{subdomain}.g.alchemy.com/...`, subscribes with
`eth_subscribe(["newHeads"])` and emits one `NewHead` per
notification into an `UnboundedReceiver`. Reconnect is handled by
`retry_with_backoff` (§8.2) so we reuse the same jittered schedule
on transient WS drops.

Functional test reuses `tests/support/ws_harness.rs` with an
`ExpectSubscribeReply { expected_method: "eth_subscribe" }` step and
two `EmitNotification` steps carrying canned
`{ "number": "0x...", "parentHash": "0x..." }` bodies, asserting the
receiver produces the matching `NewHead` values and flips to `None`
when the harness is dropped.

Composition root (`src/infra/mod.rs`) does **not** yet swap the
polling home feed for the WS adapter — promotion is a separate,
small step that §8.2 of `plan/15-backlog.md` already tracks as
"NewHeadsStream live wiring". This PR ships the adapter on a shelf
so that promotion becomes a one-line change.

### 8.7 Scope boundaries

- Cost hints intentionally ship without a UI surface; adding one is
  a future plan slice, not an §8.14 goal.
- `eth_callMany` stays deferred — the batch helper covers today's
  call pattern.
- Switching the live Home feed from polling to the WS adapter stays
  a composition-root decision owned by `plan/15-backlog.md` §8.2.
