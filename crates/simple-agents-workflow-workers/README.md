# simple-agents-workflow-workers

This crate provides the Phase 4 worker gRPC contract and a Rust client-side worker pool.

## Contract generation

- Source proto: `proto/worker.proto`
- Generation mode: `build.rs` runs `tonic-build` + `prost`
- `protoc` source: vendored via `protoc-bin-vendored` (no system `protoc` needed)

## Integration points

- `GrpcWorkerClient`: single endpoint gRPC client
- `GrpcWorkerPool`: round-robin worker pool with retry support
- Implements `simple_agents_workflow::WorkerPoolClient`, so it plugs into
  `simple_agents_workflow::WorkerPoolToolExecutor` without replacing the existing in-process pool.

## Payload compatibility

`ExecuteRequest` now carries typed payload fields (`llm_payload` / `tool_payload`) while
still populating `payload_json` as a compatibility fallback for older workers.
