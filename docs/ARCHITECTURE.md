# Architecture Overview

This document describes the current SimpleAgents architecture as implemented in the Rust workspace.

## Design Goals

- Type-safe request/response contracts.
- Modular crates with clear responsibilities.
- Provider-agnostic core orchestration.
- Pluggable routing, caching, and healing.
- Multiple language surfaces built on the same core.

## System Map

```
┌────────────────────────────────────────────────────────────┐
│                          Application                       │
└─────────────────────────────┬──────────────────────────────┘
                              │
                              ▼
┌────────────────────────────────────────────────────────────┐
│                  simple-agents-core                        │
│  SimpleAgentsClient + Routing + Cache + Healing + Middleware│
└───────────────┬──────────────┬──────────────┬───────────────┘
                │              │              │
                ▼              ▼              ▼
     simple-agents-router  simple-agents-cache  simple-agents-healing
                │                                  │
                ▼                                  ▼
        simple-agents-providers              simple-agent-type
                │                                  │
                ▼                                  ▼
           Provider APIs                     Shared types/traits
```

Language surfaces build on the core client:
- CLI (`simple-agents-cli`)
- C FFI (`simple-agents-ffi`)
- Node (`simple-agents-napi`)
- Python (`simple-agents-py`)

## Cross-Language Parity Contract

Cross-language parity is enforced with a shared fixture and CI contract runner:

- Shared fixture source: `parity-fixtures/binding_contract.json`
- Contract runner: `scripts/run-binding-contracts.sh`
- CI gate: `capability-contract-gates` in `.github/workflows/bindings-ci.yml`

See [Cross-Language Capability Matrix](/CAPABILITY_MATRIX) for required minimum behavior and CI expectations.

## Request Flow

1. Build a `CompletionRequest` in `simple-agent-type`.
2. `SimpleAgentsClient` validates and runs middleware hooks.
3. The router selects a provider based on `RoutingMode`.
4. Provider executes HTTP requests and returns a unified response.
5. Core handles cache population and optional healing.

```
CompletionRequest
   -> SimpleAgentsClient
   -> RouterEngine (RoutingMode)
   -> Provider::transform_request
   -> Provider::execute
   -> Provider::transform_response
   -> CompletionResponse / HealedJson / CoercedSchema
```

## Streaming Flow

If `CompletionRequest.stream` is set, `SimpleAgentsClient` returns a streaming outcome:

```
CompletionOutcome::Stream
  -> Stream<CompletionChunk>
  -> Middleware after_stream/on_error hooks
```

## Routing and Resilience

Routing strategies live in `simple-agents-router` and include round-robin, latency, cost, and fallback routing. The router also provides circuit breaking, health tracking, and retry policies used by core and provider flows.

## Healing and Schema Coercion

Healing is implemented in `simple-agents-healing` and wired through core completion modes:
- `CompletionMode::HealedJson` parses JSON-ish output into structured values.
- `CompletionMode::CoercedSchema` parses and coerces responses into a provided schema.

Parser and coercion behavior is configurable via `HealingSettings` in `simple-agents-core`.

## Caching

Caching is optional. If enabled, `SimpleAgentsClient` uses `Cache` to store serialized responses keyed by request contents. The default in-memory cache supports TTL and eviction.

## Metrics and Observability

Metrics live in `simple-agents-providers`. The optional `prometheus` Cargo feature adds a Prometheus exporter for request timing and retry metrics.

## Language Surfaces

- `simple-agents-ffi` exposes a C ABI for core completion flows.
- `simple-agents-napi` exposes Node bindings with standard/healed/schema modes.
- `simple-agents-py` exposes Python bindings, streaming iterators, and schema helpers.

## Reference

- Rust core systems: [Rust Core Systems](/RUST_CORE_SYSTEMS)
- Cross-language parity baseline: [Capability Matrix](/CAPABILITY_MATRIX)
- Common integration issues: [Troubleshooting](/TROUBLESHOOTING)
- Workflow timeline/replay tooling: [Workflow Debugging UX](/WORKFLOW_DEBUGGING)
- Feature inventory: [features.md (repo)](https://github.com/rishub/SimpleAgents/blob/main/features.md)
