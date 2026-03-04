# Architecture Overview

This page explains how SimpleAgents is structured in the Rust workspace and how data flows from request input to provider output.
By the end, you should be able to place each crate in the system and understand where routing, caching, healing, and parity checks are enforced.

## Prerequisites

- Familiarity with [Quick Start](/QUICKSTART)
- Basic understanding of Rust crate boundaries

## Quick Path

1. Read the system map to identify the core execution path.
2. Read the request lifecycle to understand runtime order of operations.
3. Use the parity contract section if you are touching bindings.
4. Use the reference links at the end for deeper subsystem docs.

## System Model

SimpleAgents is designed as a provider-agnostic core with optional capabilities layered around it.

| Layer | Responsibility | Main crate(s) |
|---|---|---|
| Type contract | Shared request/response types and traits | `simple-agent-type` |
| Core orchestration | Unified client execution path and options | `simple-agents-core` |
| Routing | Provider selection, retries, health/circuit behavior | `simple-agents-router` |
| Providers | API-specific request/response transforms and execution | `simple-agents-providers` |
| Caching | Response memoization and eviction/TTL policy | `simple-agents-cache` |
| Healing | JSON repair and schema coercion modes | `simple-agents-healing` |
| Language surfaces | CLI + FFI + Node + Python entry points | `simple-agents-cli`, `simple-agents-ffi`, `simple-agents-napi`, `simple-agents-py` |

High-level map:

```text
Application
  -> simple-agents-core (SimpleAgentsClient)
      -> simple-agents-router (provider selection)
      -> simple-agents-providers (provider execution)
      -> optional simple-agents-cache
      -> optional simple-agents-healing
      -> shared contracts from simple-agent-type
```

## Request Lifecycle

1. Build `CompletionRequest` from `simple-agent-type`.
2. `SimpleAgentsClient` validates request and runs middleware hooks.
3. Router selects provider by configured `RoutingMode`.
4. Provider performs `transform_request -> execute -> transform_response`.
5. Core applies optional cache write/read and optional healing mode.
6. Caller receives a typed `CompletionOutcome`.

```text
CompletionRequest
  -> SimpleAgentsClient
  -> RouterEngine
  -> Provider::transform_request
  -> Provider::execute
  -> Provider::transform_response
  -> CompletionOutcome
```

If `CompletionRequest.stream` is enabled, the final outcome is `CompletionOutcome::Stream` and chunks flow through stream-aware hooks.

## Cross-Language Parity Contract

Cross-language behavior is guarded by shared fixtures and CI contract checks.

- Shared fixture: `parity-fixtures/binding_contract.json`
- Workflow golden fixture: `parity-fixtures/workflow_dsl_ir_golden.json`
- Contract runner script: `scripts/run-binding-contracts.sh`
- CI gate: `capability-contract-gates` in `.github/workflows/bindings-ci.yml`

If you change API behavior in core crates, update contract fixtures and binding verifiers in the same change set.

## Workflow Authoring Surfaces

YAML and code-first workflow surfaces are kept aligned with parity fixtures.

- Authoring fixture: `parity-fixtures/workflow_dsl_ir_golden.json`
- Rust verifier: `crates/simple-agents-workflow/tests/workflow_dsl_ir_fixtures.rs`
- Python verifier: `crates/simple-agents-py/tests/test_contract_fixtures.py`
- Node verifier: `crates/simple-agents-napi/test/contract.test.js`
- Go verifier: `bindings/go/contract_fixture_test.go`

When migrating between YAML and code, preserve node ids and edge semantics to keep replay/debug outputs deterministic.

## Operational Capabilities

- **Routing and resilience:** latency/cost/round-robin/fallback strategies with health tracking.
- **Healing and schema coercion:** `CompletionMode::HealedJson` and `CompletionMode::CoercedSchema`.
- **Caching:** request-keyed response caching with TTL and eviction.
- **Metrics/observability:** provider metrics with optional Prometheus feature.

## Troubleshooting

### Behavior differs across bindings

Re-run parity checks and compare against `parity-fixtures/binding_contract.json` before changing public APIs.

### Workflow migration produces different graph behavior

Check node id and edge mapping against `parity-fixtures/workflow_dsl_ir_golden.json` and corresponding binding tests.

### Runtime output shape changed unexpectedly

Check whether healing mode or schema coercion mode was enabled in `CompletionOptions`.

## Next Steps

- Read crate boundaries in [Rust Core Systems](/RUST_CORE_SYSTEMS).
- Review compatibility targets in [Capability Matrix](/CAPABILITY_MATRIX).
- Learn workflow runtime details in [YAML Workflow System Guide](/YAML_WORKFLOW_SYSTEM).
- Use [Workflow Debugging UX](/WORKFLOW_DEBUGGING) for replay/timeline diagnostics.
