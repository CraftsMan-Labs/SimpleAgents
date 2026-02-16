- [x] **Task 1: Replace blocking `std::sync::RwLock` in `SimpleAgentsClient` state**
  - **Issue:** The client state (`providers`, `provider_map`, `router`) is protected with `std::sync::RwLock`, which can block the async executor when contended.
  - **Sample code (current):**
  ```rust
  // crates/simple-agents-core/src/client.rs
  pub struct SimpleAgentsClient {
      state: RwLock<ClientState>,
      routing_mode: RoutingMode,
      cache: Option<Arc<dyn Cache>>,
      cache_ttl: Duration,
      healing: HealingSettings,
      middleware: Vec<Arc<dyn Middleware>>,
  }
  ```
  - **Consequences:** Blocking locks can stall unrelated async tasks under load, increasing tail latency and risking deadlocks if held across await points in future changes.
  - **Plan / draft fix:** Switch to an async-friendly lock (e.g., `tokio::sync::RwLock` or `parking_lot::RwLock`), update imports, and adjust call sites to avoid blocking the runtime. Ensure router rebuilds remain synchronous and keep the API unchanged.
  - Subtasks:
    - [x] Swap lock type and imports in `client.rs`; ensure `ClientState` remains `Send + Sync`.
    - [x] Re-run tests to confirm no type inference regressions and no new async `await` requirements leak into the API surface.

- [x] **Task 2: Add middleware instrumentation for streaming completions**
  - **Issue:** The streaming path bypasses middleware hooks for success/error/latency and uses `eprintln!` for logging.
  - **Sample code (current):**
  ```rust
  // crates/simple-agents-core/src/client.rs
  async fn stream(&self, request: &CompletionRequest)
  ```
  ```rust
  {
      self.before_request(request).await?;
      eprintln!("SimpleAgentsClient.stream: model={}, stream={:?}", request.model, request.stream);
      let router = { /* read lock and clone router */ };
      router.stream(request).await
  }
  ```
  - **Consequences:** Missing `after_response`/`on_error` hooks means observability gaps (no metrics/tracing for streams) and middleware behaviors (circuit metrics, auditing) do not run. `eprintln!` is noisy and unstructured.
  - **Plan / draft fix:** Wrap the returned stream to trigger middleware callbacks on completion/error and measure latency; replace `eprintln!` with structured tracing (`tracing::debug!`) or remove it.
  - Subtasks:
    - [x] Introduce a wrapper stream that records start time, forwards chunks, and on termination/error invokes middleware hooks.
    - [x] Add structured logging; ensure errors propagate unchanged.
    - [x] Extend tests to cover middleware invocation for streaming.

- [x] **Task 3: Prevent duplicate provider registration in `register_provider`**
  - **Issue:** `register_provider` pushes providers without guarding against duplicate names while overwriting the map entry, leading to multiple router entries with the same name.
  - **Sample code (current):**
  ```rust
  state.provider_map.insert(provider.name().to_string(), provider.clone());
  state.providers.push(provider);
  state.router = Arc::new(self.routing_mode.build_router(state.providers.clone())?);
  ```
  - **Consequences:** Duplicate providers skew routing (round-robin/cost/latency) and create confusion when listing providers; subtle bugs if middlewares assume unique provider identity.
  - **Plan / draft fix:** Reject or replace duplicates deterministically (e.g., return `Config` error when the name already exists). Keep `provider_map` and `providers` in sync.
  - Subtasks:
    - [x] Add duplicate check and surface a clear `SimpleAgentsError::Config` message.
    - [x] Add tests covering duplicate registration and ensuring routing list matches map.

- [x] **Task 4: Reduce contention in `InMemoryCache::get` by avoiding write locks on read**
  - **Issue:** `get` takes a write lock on the cache store even for reads, serializing readers and increasing contention.
  - **Sample code (current):**
  ```rust
  // crates/simple-agents-cache/src/memory.rs
  let mut store = self.store.write().await;
  if let Some(entry) = store.get_mut(key) { /* ... */ }
  ```
  - **Consequences:** Under concurrent load, reads block each other and writers, degrading latency and throughput; unnecessary lock promotion risk.
  - **Plan / draft fix:** Use a read lock for the hot path; only acquire a write lock when removing expired entries or touching LRU metadata (can use an upgradable lock pattern or split reads/writes).
  - Subtasks:
    - [x] Refactor `get` to first read under a shared lock, then upgrade/mutate minimally for touch/removal.
    - [x] Add a concurrency-focused test (multiple `get` calls) to ensure correctness and no deadlocks.

- [x] **Task 5: Repository-wide review for hygiene, KISS/DRY, CX, and bug reporting**
  - **Scope reviewed:** core/runtime/providers/router/cache/workflow workers/bindings.
  - **Method:** static code review with focus on correctness, maintainability, API consistency, and operator/developer experience.
  - **Outcome:** documented prioritized issues and positive observations below.

## Grading

- **Scale used:** A (excellent), B (good), C (needs improvement), D (high risk), F (critical).
- **Code hygiene:** **B** (generally clean structure; some concurrency/retry edge cases remain).
- **KISS:** **B-** (most modules are readable; a few flows are over-coupled, especially worker/pool internals).
- **DRY:** **B-** (good reuse overall; retry and transport behaviors diverge across components/providers).
- **CX (developer/operator/user experience):** **C+** (good guardrails and middleware progress, but docs/config/runtime consistency gaps remain).
- **Reliability/Bug risk:** **C** (two high-priority correctness risks identified).
- **Overall grade:** **B-**.

## Findings (Prioritized)

### High

- [x] **H1: Duplicate providers can still be silently accepted at build-time**
  - **Grade:** **D**
  - **Why it matters:** Routing can include duplicate providers while `provider_map` keeps only the last value for a name, creating inconsistent behavior and skewed routing.
  - **Evidence:** `crates/simple-agents-core/src/client.rs:401`, `crates/simple-agents-core/src/client.rs:407`, `crates/simple-agents-core/src/client.rs:450`.
  - **Bug report:** `with_provider`/`with_providers` allow duplicates; map collection in `build()` overwrites by key with no validation.
  - **Recommended fix:** Validate unique provider names in `build()` and return `SimpleAgentsError::Config` on duplicates (aligned with `register_provider`).

- [x] **H2: Retry helpers can panic when `max_attempts == 0`**
  - **Grade:** **F**
  - **Why it matters:** Invalid config can trigger runtime panic instead of returning a typed error.
  - **Evidence:** `crates/simple-agents-providers/src/retry.rs:53`, `crates/simple-agents-providers/src/retry.rs:89`, `crates/simple-agents-router/src/retry.rs:66`, `crates/simple-agents-router/src/retry.rs:85`, `crates/simple-agent-type/src/config.rs:12`.
  - **Bug report:** `Err(last_error.unwrap())` can panic if loop never runs.
  - **Recommended fix:** Enforce `max_attempts >= 1` at config boundaries and guard retry helpers with a typed config error when zero.

### Medium

- [x] **M1: gRPC schema drift risk in Go worker**
  - **Grade:** **C**
  - **Why it matters:** Proto evolution can diverge from manually constructed descriptors, causing protocol mismatch.
  - **Evidence:** `crates/simple-agents-workflow-workers/proto/worker.proto:18`, `workers/go/worker.go:143`.
  - **Issue:** `metadata` exists in proto but is not represented in manual descriptor fields.
  - **Recommended fix:** Generate Go bindings from `worker.proto` to keep one source of truth.

- [x] **M2: `GrpcWorkerPool` retries effectively disabled for single-worker pools**
  - **Grade:** **C-**
  - **Why it matters:** Transient failures on a single endpoint are not retried despite configured retries.
  - **Evidence:** `crates/simple-agents-workflow-workers/src/pool.rs:16`, `crates/simple-agents-workflow-workers/src/pool.rs:82`.
  - **Issue:** Attempt cap uses worker count, so `len == 1` results in one attempt.
  - **Recommended fix:** Base attempts on `max_retries + 1` and permit retrying same worker when pool size is 1 (optional jitter/backoff).

- [x] **M3: Await while holding pool-wide mutex in worker selection path**
  - **Grade:** **C-**
  - **Why it matters:** Increases contention and deadlock risk under future changes.
  - **Evidence:** `crates/simple-agents-workflow/src/worker.rs:467`, `crates/simple-agents-workflow/src/worker.rs:481`, `crates/simple-agents-workflow/src/worker.rs:486`.
  - **Issue:** Async operations occur while `slots` mutex is held.
  - **Recommended fix:** Snapshot required data, drop lock, then perform async health/hook calls.

- [x] **M4: `Retry-After` support is incomplete and not wired end-to-end**
  - **Grade:** **C**
  - **Why it matters:** Clients may ignore backoff hints and amplify rate-limit errors.
  - **Evidence:** `crates/simple-agents-providers/src/utils.rs:32`, `crates/simple-agents-providers/src/openai/error.rs:84`, `crates/simple-agents-providers/src/anthropic/error.rs:78`.
  - **Issue:** Parser only handles integer seconds and is not integrated into provider error mapping.
  - **Recommended fix:** Parse seconds and HTTP-date variants, plumb into provider errors, and apply in retry sleep policy.

- [x] **M5: Inconsistent HTTP client defaults across providers**
  - **Grade:** **C+**
  - **Why it matters:** Mixed protocol behavior reduces portability with proxies/custom endpoints and complicates support.
  - **Evidence:** `crates/simple-agents-providers/src/anthropic/mod.rs:91`, `crates/simple-agents-providers/src/anthropic/mod.rs:111`, `crates/simple-agents-providers/src/openrouter/mod.rs:127`, `crates/simple-agents-providers/src/openai/mod.rs:84`.
  - **Issue:** Some providers force HTTP/2 prior knowledge while others rely on negotiated transport.
  - **Recommended fix:** Standardize on ALPN negotiation by default; keep forced prior-knowledge opt-in.

### Low (CX / Docs)

- [x] **L1: README version appears stale vs workspace metadata**
  - **Grade:** **B-**
  - **Why it matters:** Documentation mismatch harms developer trust and onboarding clarity.
  - **Evidence:** `README.md:80`, `Cargo.toml:6`.
  - **Recommended fix:** Update README versioning guidance and/or add CI check for doc-version consistency.

- [x] **L2: TypeScript worker identity/config is hardcoded**
  - **Grade:** **C+**
  - **Why it matters:** Weak operability for multi-instance deployment and lower parity with other workers.
  - **Evidence:** `workers/typescript/worker.ts:49`, `workers/typescript/worker.ts:54`, `workers/typescript/worker.ts:70`.
  - **Recommended fix:** Add CLI/env config for `worker-id` and listen address.

## Positive Notes

- [x] Streaming middleware instrumentation is integrated and improves observability: `crates/simple-agents-core/src/client.rs:245`, `crates/simple-agents-core/src/client.rs:347`.
- [x] Cache read path now reduces contention and includes concurrency-oriented validation: `crates/simple-agents-cache/src/memory.rs:136`, `crates/simple-agents-cache/src/memory.rs:351`.
- [x] Worker runtime validation includes strong guardrails and limits: `crates/simple-agents-workflow/src/worker.rs:639`.

## Recommended Next Fix Order

1. H2 panic guard (`max_attempts == 0`).
2. H1 duplicate provider validation in builder path.
3. M2 retry behavior for single-worker pools.
4. M3 mutex/await separation in worker selection.
5. M4 Retry-After end-to-end wiring.

## Next Steps to Get These Fixed

### Phase 1 (Immediate: correctness and panic safety)

- [x] **Step 1: Patch H2 first (panic prevention)**
  - Implement `max_attempts >= 1` validation in config construction and deserialization boundaries.
  - Replace `unwrap()`-based terminal paths with explicit typed errors in retry helpers.
  - Add tests: `max_attempts = 0` should return config error (never panic), `max_attempts = 1` should still behave correctly.
  - **Definition of done:** No panic path remains for retry attempt count; tests cover guardrails.

- [x] **Step 2: Patch H1 next (provider uniqueness invariant)**
  - Add duplicate-name validation in builder flow before router construction.
  - Keep behavior consistent with `register_provider` by returning `SimpleAgentsError::Config`.
  - Add tests for `with_provider` and `with_providers` duplicate cases.
  - **Definition of done:** provider list and provider map cannot diverge by duplicate names.

### Phase 2 (Reliability and concurrency)

- [x] **Step 3: Fix M2 retry semantics for single-worker pools**
  - Compute attempts as `max_retries + 1` independent of pool size.
  - Allow retries on the same worker when pool size is one; add optional small jitter/backoff.
  - Add tests for one-worker transient failures and eventual success/final failure behavior.
  - **Definition of done:** configured retries are honored for both single and multi-worker pools.

- [x] **Step 4: Fix M3 lock/await coupling in worker selection**
  - Refactor to snapshot slot state under lock, release lock, then run async health/hook calls.
  - Add a contention test (parallel selection + health checks) to verify no lock starvation/deadlock.
  - **Definition of done:** no `.await` occurs while holding pool-wide mutex.

### Phase 3 (Resilience and DX/CX consistency)

- [x] **Step 5: Complete M4 Retry-After support end-to-end**
  - Extend parser to support both integer seconds and HTTP-date forms.
  - Propagate parsed values through provider error mapping and retry policies.
  - Add tests for 429 responses with both header formats.
  - **Definition of done:** client honors server backoff hints consistently.

- [x] **Step 6: Address M1/M5/L1/L2 cleanup items**
  - M1: move Go worker to generated proto bindings.
  - M5: standardize provider transport defaults (ALPN by default; prior-knowledge opt-in).
  - L1: align README versioning with workspace metadata and add CI drift check.
  - L2: add TypeScript worker `worker-id` and listen address CLI/env config.
  - **Definition of done:** protocol and configuration behavior are consistent across workers/providers/docs.

## Suggested Delivery Cadence

- **PR 1 (high-priority):** H2 + H1 + tests.
- **PR 2 (runtime reliability):** M2 + M3 + tests/bench checks.
- **PR 3 (resilience + DX):** M4 + M1 + M5 + L1 + L2.
- **Validation gates for each PR:** `cargo test --workspace`, targeted integration tests for touched crates, and changelog/docs update when behavior changes.

## How to Maintain Grade-Quality Code (Ongoing)

### Quality Bar (Team Standard)

- [ ] **Set explicit release gate:** no new panic paths, no `.unwrap()`/`.expect()` in runtime paths (except tests), and no duplicate config invariants.
- [ ] **Require evidence with every PR:** tests for bug fix + one regression case + short note describing risk and rollback.
- [ ] **Keep KISS as policy:** prefer smaller functions and single-responsibility modules; reject broad refactors mixed with behavior changes.
- [ ] **Keep DRY with intent:** centralize shared retry/transport behavior in common utilities instead of per-provider drift.

### CI and Automation Guardrails

- [ ] **Static checks in CI:** `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test --workspace`.
- [ ] **Risk-focused checks:** add custom lint/check script for `unwrap/expect` in non-test runtime crates.
- [ ] **Doc consistency checks:** fail CI when README/version references drift from `Cargo.toml` workspace version.
- [ ] **Protocol safety checks:** ensure generated bindings are up-to-date (proto generation verification in CI for worker interfaces).

### Code Review Checklist (Fast, Repeatable)

- [ ] **Correctness:** can malformed input/config cause panic or undefined behavior?
- [ ] **Concurrency:** any `.await` while holding mutex/RwLock or lock upgrades that can starve?
- [ ] **Resilience:** retries/backoff/hints (e.g., `Retry-After`) handled consistently?
- [ ] **Consistency:** same behavior across providers/workers for transport, errors, and config defaults?
- [ ] **CX impact:** will users/operators understand failure mode and recovery action from error messages/docs?

### Testing Strategy to Sustain Grades

- [ ] **Test pyramid by change type:** unit tests for logic, integration tests for crate boundaries, scenario tests for worker/provider behavior.
- [ ] **Regression-first approach:** when bug found, write failing test first, then fix, then keep test permanently.
- [ ] **Concurrency coverage:** include targeted parallel tests for cache/pool/worker scheduling hotspots.
- [ ] **Contract tests:** lock API/proto/error-shape behavior so refactors do not break downstream consumers.

### Operational Feedback Loop

- [ ] **Track quality KPIs per release:** panic count, retry success rate, 429 recovery rate, flaky test count, and bug reopen rate.
- [ ] **Do lightweight retros every release:** top 3 regressions, why they escaped, and one preventive rule added.
- [ ] **Maintain an optimization backlog:** keep this file updated with grades before/after each fix batch.
- [ ] **Re-grade monthly:** rerun this review rubric and compare deltas to ensure trend is improving (target: move overall from **B-** to **A-**).

## Completion Update (2026-02-16)

- [x] High/medium/low findings in this review were implemented and validated.
- [x] Rust validation run completed for changed crates: `simple-agent-type`, `simple-agents-core`, `simple-agents-providers`, `simple-agents-workflow`, `simple-agents-workflow-workers`.
- [x] Go worker validation completed via `go test ./...` in `workers/go`.
- [x] M1 drift-risk fix uses `worker.proto` as runtime source-of-truth (descriptor parsed from proto) and removes manual descriptor construction.
