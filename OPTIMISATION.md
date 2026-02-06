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
