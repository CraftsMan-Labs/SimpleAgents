# Async Tokio Patterns

## Table of Contents

1. Runtime Setup
2. Task Orchestration Patterns
3. Channel Selection Guide
4. Stream Patterns
5. Graceful Shutdown and Cancellation
6. Async Error Handling Patterns
7. Async Trait Patterns
8. Shared State and Resource Management
9. Debugging and Observability
10. Async Anti-Patterns

## 1. Runtime Setup

Default stack for production async services:

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
futures = "0.3"
async-trait = "0.1"
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
tokio-util = "0.7"
```

Runtime bootstrap:

```rust
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_target(false).init();
    run().await
}
```

## 2. Task Orchestration Patterns

### `JoinSet` for dynamic fan-out

Use when number of tasks is discovered at runtime.

```rust
use tokio::task::JoinSet;

async fn fetch_all(urls: Vec<String>) -> Vec<anyhow::Result<String>> {
    let mut set = JoinSet::new();
    for url in urls {
        set.spawn(async move { fetch(&url).await });
    }

    let mut out = Vec::new();
    while let Some(res) = set.join_next().await {
        match res {
            Ok(value) => out.push(value),
            Err(e) => out.push(Err(anyhow::anyhow!("join failure: {e}"))),
        }
    }
    out
}
```

### Bounded concurrency with `buffer_unordered`

Use for throughput without unbounded task growth.

```rust
use futures::stream::{self, StreamExt};

async fn fetch_limited(urls: Vec<String>, limit: usize) -> Vec<anyhow::Result<String>> {
    stream::iter(urls)
        .map(|url| async move { fetch(&url).await })
        .buffer_unordered(limit)
        .collect()
        .await
}
```

### `tokio::select!` for cancellation/timeouts/racing

Use for multi-condition control flow.

## 3. Channel Selection Guide

- `mpsc`: work queues, ingestion pipelines, producer-worker model.
- `broadcast`: pub/sub event fan-out.
- `oneshot`: request/response handoff of one value.
- `watch`: config/state latest-value propagation.

Decision defaults:
- Need backpressure and ordered ingestion: `mpsc` with bounded capacity.
- Need each subscriber to observe events: `broadcast`.
- Need consumers to read only current state: `watch`.

## 4. Stream Patterns

Common patterns:
- Use `StreamExt::chunks` for batch processing.
- Use `select` to merge streams.
- Use combinators to avoid materializing intermediate collections.

## 5. Graceful Shutdown and Cancellation

Preferred strategy:

1. Create a root `CancellationToken`.
2. Clone token into all long-running tasks.
3. Use `tokio::select!` with `token.cancelled()` inside task loops.
4. Trigger cancellation on `ctrl_c`/health signal.
5. Wait bounded time for cleanup.

```rust
use tokio::signal;
use tokio::time::{sleep, Duration};
use tokio_util::sync::CancellationToken;

async fn run_service() -> anyhow::Result<()> {
    let token = CancellationToken::new();
    let task_token = token.clone();

    let handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = task_token.cancelled() => break,
                _ = do_work_cycle() => {}
            }
        }
    });

    signal::ctrl_c().await?;
    token.cancel();

    tokio::select! {
        _ = handle => {},
        _ = sleep(Duration::from_secs(5)) => {
            tracing::warn!("shutdown timeout");
        }
    }

    Ok(())
}
```

## 6. Async Error Handling Patterns

- Library crates: define typed errors with `thiserror`.
- Binaries/services: use `anyhow::Result` at orchestration layers.
- Add context at I/O boundaries and external calls.
- Classify retryable vs non-retryable failures.
- Wrap uncertain latency operations with explicit timeout.

## 7. Async Trait Patterns

- Use `async-trait` when dynamic dispatch or object-safe trait APIs are required.
- Prefer generic traits where possible for static dispatch.
- Keep trait methods narrow and transport-agnostic.

## 8. Shared State and Resource Management

- Prefer message passing over shared mutable state when feasible.
- For shared state:
  - Read-heavy: `Arc<RwLock<T>>`
  - Write-heavy or simple critical sections: `Arc<Mutex<T>>`
- Never hold lock guards across `.await`.
- Bound scarce resources with semaphores.

## 9. Debugging and Observability

- Add `#[tracing::instrument]` on async boundaries.
- Emit structured fields (`request_id`, `user_id`, `task_id`).
- Correlate spawned tasks with spans.
- Use Tokio console for runtime task visibility when needed.

## 10. Async Anti-Patterns

- Blocking executor threads (`std::thread::sleep`, blocking DB clients).
- Unbounded spawn loops under uncontrolled input.
- Swallowing join errors.
- Mixing cancellation-agnostic tasks with graceful shutdown guarantees.
