# Concurrency and Backpressure

## Recommended Repo
- https://github.com/tokio-rs/tokio

## Why This Repo
- De-facto Rust async runtime with primitives for backpressure and concurrency control.
- Provides patterns for worker pools and max in-flight control.

## Pros
- High-performance async runtime with mature ecosystem.
- Native support for channels, streams, and backpressure patterns.

## Cons
- Requires careful tuning to avoid head-of-line blocking.
- Complex concurrency patterns can be hard to reason about.

## What We Want To Build From This
- Worker pools with shared max in-flight limits.
- Backpressure-aware stream processing for node outputs.
- Structured concurrency for fan-out/fan-in.

## Why
- We need predictable throughput without spawning per-request processes.

## Sources
- https://tokio.rs/tokio/tutorial/streams
- https://users.rust-lang.org/t/async-rust-with-tokio-i-o-streams-backpressure-concurrency-and-ergonomics/134465

## Notes
- Use Tokio streams and channels as reference for backpressure design.
