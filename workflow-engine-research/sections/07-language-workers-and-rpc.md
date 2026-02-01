# Language Workers and RPC

## Recommended Repo
- https://github.com/hyperium/tonic

## Why This Repo
- Rust-native gRPC implementation suitable for cross-language worker RPC.
- Strong documentation and ecosystem for code generation and streaming.

## Pros
- Cross-language interoperability and streaming support.
- Strong Rust ecosystem integration and tooling.

## Cons
- gRPC adds runtime overhead and schema management complexity.
- Requires protobuf schema management across languages.

## What We Want To Build From This
- A stable worker RPC contract for Rust/Python/Go/TS workers.
- Streaming results and structured error propagation.
- Versioned RPC schema for backwards compatibility.

## Why
- Long-lived workers need a consistent cross-language interface.

## Sources
- https://docs.rs/tonic/latest/tonic/
- https://generalistprogrammer.com/tutorials/tonic-rust-crate-guide

## Notes
- Use gRPC as the default RPC contract for long-lived workers.
